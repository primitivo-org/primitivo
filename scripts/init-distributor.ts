import * as anchor from "@coral-xyz/anchor";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { PublicKey } from "@solana/web3.js";
import { createAccount, createMint, mintTo, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { getRoot, loadEntries } from "../merkle-tree-generator/src/merkle";

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

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const authority = provider.wallet;

  const airdropFile = requireArg("--airdrop-file");
  const idArg = getArg("--id") ?? "1";
  const decimalsArg = getArg("--decimals") ?? "6";
  const createMintFlag = hasFlag("--create-mint");
  const mintArg = getArg("--mint");
  const sourceTokenAccountArg = getArg("--source-token-account");
  const fundingAmountArg = getArg("--funding-amount");

  const idBigInt = BigInt(idArg);
  const idBn = toU64(idBigInt, "id");
  const decimals = Number(decimalsArg);
  if (!Number.isInteger(decimals) || decimals < 0 || decimals > 9) {
    throw new Error("--decimals must be an integer between 0 and 9");
  }

  const entries = loadEntries(resolve(airdropFile));
  if (entries.length === 0) {
    throw new Error("Airdrop list is empty");
  }

  const merkleRoot = getRoot(entries);
  const totalFromList = entries.reduce((sum, e) => sum + e.amount, 0n);
  const fundingAmount = fundingAmountArg ? BigInt(fundingAmountArg) : totalFromList;

  let mint: PublicKey;
  let sourceTokenAccount: PublicKey;

  if (createMintFlag) {
    mint = await createMint(
      provider.connection,
      (authority as any).payer,
      authority.publicKey,
      null,
      decimals,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID,
    );

    sourceTokenAccount = await createAccount(
      provider.connection,
      (authority as any).payer,
      mint,
      authority.publicKey,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID,
    );

    if (fundingAmount > 0n) {
      await mintTo(
        provider.connection,
        (authority as any).payer,
        mint,
        sourceTokenAccount,
        authority.publicKey,
        toU64(fundingAmount, "funding amount").toNumber(),
        [],
        undefined,
        TOKEN_PROGRAM_ID,
      );
    }
  } else {
    if (!mintArg) {
      throw new Error("--mint is required when not using --create-mint");
    }
    if (!sourceTokenAccountArg) {
      throw new Error("--source-token-account is required when not using --create-mint");
    }

    mint = new PublicKey(mintArg);
    sourceTokenAccount = new PublicKey(sourceTokenAccountArg);
  }

  const programIdArg = getArg("--program-id");
  const programId = programIdArg ? new PublicKey(programIdArg) : new PublicKey(requireArg("--program-id"));

  const idlPath = resolve("target/idl/merke_airdrop.json");
  const idl = JSON.parse(readFileSync(idlPath, "utf8"));
  const program = new anchor.Program(idl, provider) as anchor.Program<any>;

  if (!program.programId.equals(programId)) {
    throw new Error(
      `Program ID mismatch. IDL/program: ${program.programId.toBase58()}, --program-id: ${programId.toBase58()}`,
    );
  }

  const [distributorPda] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("distributor"),
      authority.publicKey.toBuffer(),
      mint.toBuffer(),
      idBn.toArrayLike(Buffer, "le", 8),
    ],
    program.programId,
  );

  const [vaultPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), distributorPda.toBuffer()],
    program.programId,
  );
  const [claimBitmapPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("bitmap"), distributorPda.toBuffer()],
    program.programId,
  );

  const txSig = await program.methods
    .initializeDistributor(
      idBn,
      [...merkleRoot] as number[],
      entries.length,
      toU64(fundingAmount, "funding amount"),
    )
    .accountsPartial({
      authority: authority.publicKey,
      mint,
      sourceTokenAccount,
      distributor: distributorPda,
      vault: vaultPda,
      claimBitmap: claimBitmapPda,
      systemProgram: anchor.web3.SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .rpc();

  console.log(
    JSON.stringify(
      {
        txSig,
        authority: authority.publicKey.toBase58(),
        programId: program.programId.toBase58(),
        distributorId: idBn.toString(),
        distributor: distributorPda.toBase58(),
        vault: vaultPda.toBase58(),
        claimBitmap: claimBitmapPda.toBase58(),
        maxClaims: entries.length,
        mint: mint.toBase58(),
        sourceTokenAccount: sourceTokenAccount.toBase58(),
        fundingAmount: fundingAmount.toString(),
        totalFromList: totalFromList.toString(),
        merkleRoot: `0x${Buffer.from(merkleRoot).toString("hex")}`,
        entries: entries.length,
      },
      null,
      2,
    ),
  );
}

main().catch((err) => {
  console.error(err instanceof Error ? err.message : err);
  console.error(`\nUsage:\n  npm run init:distributor -- --program-id <PROGRAM_ID> --airdrop-file <PATH> [--id 1] [--funding-amount <u64>] [--create-mint --decimals 6] [--mint <MINT> --source-token-account <TOKEN_ACCOUNT>]\n`);
  process.exit(1);
});
