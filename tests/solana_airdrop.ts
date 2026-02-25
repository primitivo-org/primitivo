import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import { TOKEN_PROGRAM_ID, createAccount, createMint, getAccount, mintTo } from "@solana/spl-token";
import { Keypair, PublicKey } from "@solana/web3.js";
import { assert } from "chai";
import { SolanaAirdrop } from "../target/types/solana_airdrop";
import { AirdropEntry, getProof, getRoot } from "../merkle-tree-generator/src/merkle";

type TestUser = {
  keypair: Keypair;
  amount: bigint;
};

describe("solana_airdrop integration", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const provider = anchor.getProvider() as anchor.AnchorProvider;
  const program = anchor.workspace.SolanaAirdrop as Program<SolanaAirdrop>;
  const authority = provider.wallet;

  let users: TestUser[] | undefined;
  let entries: AirdropEntry[] | undefined;
  let merkleRoot: Buffer | undefined;
  let distributorId: anchor.BN | undefined;
  let totalFundingAmount: anchor.BN | undefined;
  let mint: PublicKey | undefined;
  let sourceTokenAccount: PublicKey | undefined;
  let distributorPda: PublicKey | undefined;
  let vaultPda: PublicKey | undefined;

  let claimant: TestUser | undefined;
  let claimantAmountBn: anchor.BN | undefined;
  let claimantTokenAccount: PublicKey | undefined;
  let claimReceiptPda: PublicKey | undefined;
  let proof: Buffer[] | undefined;

  it("step 1: create users and merkle root", async () => {
    users = Array.from({ length: 5 }, (_, i) => ({
      keypair: Keypair.generate(),
      amount: BigInt(500_000 + i * 100_000),
    }));

    entries = users.map((u) => ({
      address: u.keypair.publicKey.toBase58(),
      amount: u.amount,
    }));

    merkleRoot = getRoot(entries);
    distributorId = new anchor.BN(1);
    totalFundingAmount = new anchor.BN(users.reduce((sum, u) => sum + Number(u.amount), 0));

    mint = await createMint(
      provider.connection,
      (authority as any).payer,
      authority.publicKey,
      null,
      6,
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

    await mintTo(
      provider.connection,
      (authority as any).payer,
      mint,
      sourceTokenAccount,
      authority.publicKey,
      totalFundingAmount.toNumber(),
      [],
      undefined,
      TOKEN_PROGRAM_ID,
    );

    [distributorPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("distributor"),
        authority.publicKey.toBuffer(),
        mint.toBuffer(),
        distributorId.toArrayLike(Buffer, "le", 8),
      ],
      program.programId,
    );

    [vaultPda] = PublicKey.findProgramAddressSync([Buffer.from("vault"), distributorPda.toBuffer()], program.programId);
  });

  it("step 2: initialize distributor", async function () {
    if (!distributorId || !merkleRoot || !totalFundingAmount || !mint || !sourceTokenAccount || !distributorPda || !vaultPda) {
      this.skip();
      return;
    }

    const ix = await program.methods
      .initializeDistributor(distributorId, [...merkleRoot] as number[], totalFundingAmount)
      .accountsPartial({
        authority: authority.publicKey,
        mint,
        sourceTokenAccount,
        distributor: distributorPda,
        vault: vaultPda,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();

    await provider.sendAndConfirm(new web3.Transaction().add(ix));
  });

  it("step 3: prepare claim proof and accounts", async function () {
    if (!users || !entries || !distributorPda || !mint) {
      this.skip();
      return;
    }

    claimant = users[2];
    claimantAmountBn = new anchor.BN(claimant.amount.toString());

    const airdropSig = await provider.connection.requestAirdrop(claimant.keypair.publicKey, 2 * anchor.web3.LAMPORTS_PER_SOL);
    const latest = await provider.connection.getLatestBlockhash();
    await provider.connection.confirmTransaction({ signature: airdropSig, ...latest }, "confirmed");

    claimantTokenAccount = await createAccount(
      provider.connection,
      (authority as any).payer,
      mint,
      claimant.keypair.publicKey,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID,
    );

    const generatedProof = getProof(entries, claimant.keypair.publicKey.toBase58());
    proof = generatedProof.proof;
    assert.equal(generatedProof.amount.toString(), claimant.amount.toString());

    [claimReceiptPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("claim"), distributorPda.toBuffer(), claimant.keypair.publicKey.toBuffer()],
      program.programId,
    );
  });

  it("step 4: execute claim", async function () {
    if (!claimant || !claimantAmountBn || !proof || !claimantTokenAccount || !claimReceiptPda || !distributorPda || !mint || !vaultPda) {
      this.skip();
      return;
    }
    
    await program.methods
      .claim(claimantAmountBn, proof.map((p) => [...p] as number[]))
      .accountsPartial({
        claimant: claimant.keypair.publicKey,
        distributor: distributorPda,
        mint,
        vault: vaultPda,
        claimantTokenAccount,
        claimReceipt: claimReceiptPda,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([claimant.keypair])
      .rpc();
  });

  it("step 5: assert claimant received expected amount", async function () {
    if (!claimant || !claimantTokenAccount) {
      this.skip();
      return;
    }

    const claimantToken = await getAccount(provider.connection, claimantTokenAccount, undefined, TOKEN_PROGRAM_ID);
    assert.equal(Number(claimantToken.amount), Number(claimant.amount));
  });

  it("step 6: assert double claim fails", async function () {
    if (!claimant || !claimantAmountBn || !proof || !claimantTokenAccount || !claimReceiptPda || !distributorPda || !mint || !vaultPda) {
      this.skip();
      return;
    }

    try {
      await program.methods
        .claim(claimantAmountBn, proof.map((p) => [...p] as number[]))
        .accountsPartial({
          claimant: claimant.keypair.publicKey,
          distributor: distributorPda,
          mint,
          vault: vaultPda,
          claimantTokenAccount,
          claimReceipt: claimReceiptPda,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([claimant.keypair])
        .rpc();
      assert.fail("double claim should fail");
    } catch (e) {
      assert.include(`${e}`, "already in use");
    }
  });
});
