import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import { TOKEN_PROGRAM_ID, createAccount, createMint, getAccount, mintTo } from "@solana/spl-token";
import { Keypair, PublicKey } from "@solana/web3.js";
import { assert } from "chai";
import { MerkeAirdrop } from "../target/types/merke_airdrop";
import { AirdropEntry, getProof, getRoot } from "../utils/merkle-tree-generator/src/merkle";

type TestUser = {
  keypair: Keypair;
  amount: bigint;
};

describe("merke_airdrop integration", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const provider = anchor.getProvider() as anchor.AnchorProvider;
  const program = anchor.workspace.MerkeAirdrop as Program<MerkeAirdrop>;
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
  let claimBitmapPda: PublicKey | undefined;
  let maxClaims: number | undefined;

  let claimant: TestUser | undefined;
  let claimantIndex: number | undefined;
  let claimantAmountBn: anchor.BN | undefined;
  let claimantTokenAccount: PublicKey | undefined;
  let proof: Buffer[] | undefined;

  it("step 1: create users and merkle root", async () => {
    users = Array.from({ length: 50 }, (_, i) => ({
      keypair: Keypair.generate(),
      amount: BigInt(500_000 + i * 100_000),
    }));

    entries = users.map((u) => ({
      address: u.keypair.publicKey.toBase58(),
      amount: u.amount,
    }));

    merkleRoot = getRoot(entries);
    distributorId = new anchor.BN(1);
    maxClaims = users.length;
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
    [claimBitmapPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("bitmap"), distributorPda.toBuffer()],
      program.programId,
    );
  });

  it("step 2: initialize distributor", async function () {
    if (!distributorId || !merkleRoot || !maxClaims || !totalFundingAmount || !mint || !sourceTokenAccount || !distributorPda || !vaultPda || !claimBitmapPda) {
      this.skip();
      return;
    }

    const ix = await program.methods
      .initializeDistributor(distributorId, [...merkleRoot] as number[], maxClaims, totalFundingAmount)
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
      .instruction();

    await provider.sendAndConfirm(new web3.Transaction().add(ix));
  });

  it("step 3: prepare claim proof and accounts", async function () {
    if (!users || !entries || !distributorPda || !mint) {
      this.skip();
      return;
    }

    claimantIndex = 40;
    claimant = users[claimantIndex];
    claimantAmountBn = new anchor.BN(claimant.amount.toString());

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
    assert.equal(generatedProof.index, claimantIndex);
    assert.equal(generatedProof.amount.toString(), claimant.amount.toString());
  });

  it("step 4: execute claim", async function () {
    if (!claimant || claimantIndex === undefined || !claimantAmountBn || !proof || !claimantTokenAccount || !distributorPda || !claimBitmapPda || !mint || !vaultPda) {
      this.skip();
      return;
    }

    await program.methods
      .claim(claimantIndex, claimantAmountBn, proof.map((p) => [...p] as number[]))
      .accountsPartial({
        claimant: claimant.keypair.publicKey,
        distributor: distributorPda,
        mint,
        vault: vaultPda,
        claimantTokenAccount,
        claimBitmap: claimBitmapPda,
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
    if (!claimant || claimantIndex === undefined || !claimantAmountBn || !proof || !claimantTokenAccount || !distributorPda || !claimBitmapPda || !mint || !vaultPda) {
      this.skip();
      return;
    }

    try {
      await program.methods
        .claim(claimantIndex, claimantAmountBn, proof.map((p) => [...p] as number[]))
        .accountsPartial({
          claimant: claimant.keypair.publicKey,
          distributor: distributorPda,
          mint,
          vault: vaultPda,
          claimantTokenAccount,
          claimBitmap: claimBitmapPda,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([claimant.keypair])
        .rpc();
      assert.fail("double claim should fail");
    } catch (e) {
      assert.include(`${e}`, "AlreadyClaimed");
    }
  });
});
