import * as anchor from "@coral-xyz/anchor";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { PublicKey } from "@solana/web3.js";
import { createAccount, TOKEN_PROGRAM_ID } from "@solana/spl-token";

type ProofFile = {
  address?: string;
  amount: string | number;
  proof: string[];
};

function getArg(flag: string): string | undefined {
  const idx = process.argv.indexOf(flag);
  if (idx === -1) return undefined;
  return process.argv[idx + 1];
}

function hasFlag(flag: string): boolean {
  return process.argv.includes(flag);
}

function requireArg(flag: string): string {
  const value = getArg(flag);
  if (!value || value.startsWith("--")) {
    throw new Error(`Missing required argument: ${flag}`);
  }
  return value;
}

function toU64(value: bigint, label: string): anchor.BN {
  const max = (1n << 64n) - 1n;
  if (value < 0n || value > max) {
    throw new Error(`${label} must fit in u64`);
  }
  return new anchor.BN(value.toString());
}

function hexTo32Bytes(hex: string): number[] {
  const normalized = hex.startsWith("0x") ? hex.slice(2) : hex;
  if (normalized.length !== 64) {
    throw new Error(`Invalid proof element length: expected 32 bytes, got ${normalized.length / 2}`);
  }
  const bytes = Buffer.from(normalized, "hex");
  if (bytes.length !== 32) {
    throw new Error("Invalid proof element encoding");
  }
  return [...bytes];
}

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const claimant = provider.wallet;

  const programId = new PublicKey(requireArg("--program-id"));
  const distributor = new PublicKey(requireArg("--distributor"));
  const proofFilePath = requireArg("--proof-file");
  const claimantTokenAccountArg = getArg("--claimant-token-account");
  const createTokenAccountFlag = hasFlag("--create-token-account");

  const proofFile = JSON.parse(readFileSync(resolve(proofFilePath), "utf8")) as ProofFile;
  if (!Array.isArray(proofFile.proof)) {
    throw new Error("Invalid proof file: 'proof' must be an array");
  }

  if (proofFile.address && proofFile.address !== claimant.publicKey.toBase58()) {
    throw new Error(
      `Proof address (${proofFile.address}) does not match claimant wallet (${claimant.publicKey.toBase58()})`,
    );
  }

  const amountBn = toU64(BigInt(proofFile.amount), "amount");
  const proof = proofFile.proof.map(hexTo32Bytes);

  const idlPath = resolve("target/idl/solana_airdrop.json");
  const idl = JSON.parse(readFileSync(idlPath, "utf8"));
  const program = new anchor.Program(idl, provider) as anchor.Program<any>;

  if (!program.programId.equals(programId)) {
    throw new Error(
      `Program ID mismatch. IDL/program: ${program.programId.toBase58()}, --program-id: ${programId.toBase58()}`,
    );
  }

  const distributorAccount = (await program.account.distributor.fetch(distributor)) as {
    mint: PublicKey;
    vault: PublicKey;
  };

  let claimantTokenAccount: PublicKey;
  if (claimantTokenAccountArg) {
    claimantTokenAccount = new PublicKey(claimantTokenAccountArg);
  } else if (createTokenAccountFlag) {
    claimantTokenAccount = await createAccount(
      provider.connection,
      (claimant as any).payer,
      distributorAccount.mint,
      claimant.publicKey,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID,
    );
  } else {
    throw new Error("Provide --claimant-token-account or use --create-token-account");
  }

  const [claimReceipt] = PublicKey.findProgramAddressSync(
    [Buffer.from("claim"), distributor.toBuffer(), claimant.publicKey.toBuffer()],
    program.programId,
  );

  const txSig = await program.methods
    .claim(amountBn, proof)
    .accountsPartial({
      claimant: claimant.publicKey,
      distributor,
      mint: distributorAccount.mint,
      vault: distributorAccount.vault,
      claimantTokenAccount,
      claimReceipt,
      systemProgram: anchor.web3.SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .rpc();

  console.log(
    JSON.stringify(
      {
        txSig,
        claimant: claimant.publicKey.toBase58(),
        distributor: distributor.toBase58(),
        mint: distributorAccount.mint.toBase58(),
        vault: distributorAccount.vault.toBase58(),
        claimantTokenAccount: claimantTokenAccount.toBase58(),
        claimReceipt: claimReceipt.toBase58(),
        amount: amountBn.toString(),
        proofLen: proof.length,
      },
      null,
      2,
    ),
  );
}

main().catch((err) => {
  console.error(err instanceof Error ? err.message : err);
  console.error(
    `\nUsage:\n  npm run claim -- --program-id <PROGRAM_ID> --distributor <DISTRIBUTOR_PDA> --proof-file <PROOF_JSON> [--claimant-token-account <TOKEN_ACCOUNT> | --create-token-account]\n`,
  );
  process.exit(1);
});
