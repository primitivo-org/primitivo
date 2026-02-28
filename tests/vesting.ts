import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import { TOKEN_PROGRAM_ID, createAccount, createMint, getAccount, mintTo } from "@solana/spl-token";
import { Keypair, PublicKey } from "@solana/web3.js";
import { assert } from "chai";
import { Vesting } from "../target/types/vesting";

describe("vesting integration", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const provider = anchor.getProvider() as anchor.AnchorProvider;
  const program = anchor.workspace.Vesting as Program<Vesting>;
  const authority = provider.wallet;

  let beneficiary: Keypair | undefined;
  let mint: PublicKey | undefined;
  let sourceTokenAccount: PublicKey | undefined;
  let configPda: PublicKey | undefined;
  let vaultPda: PublicKey | undefined;
  let schedulePda: PublicKey | undefined;
  let beneficiaryTokenAccount: PublicKey | undefined;

  const vestingId = new anchor.BN(1);
  const totalAmount = new anchor.BN(1_000_000);

  let startTs: anchor.BN | undefined;
  let cliffTs: anchor.BN | undefined;
  let endTs: anchor.BN | undefined;

  let firstClaimAmount = 0;
  let sourceAmountAfterRevoke = 0;

  it("step 1: setup mint, accounts, and PDAs", async () => {
    beneficiary = Keypair.generate();

    const transferIx = web3.SystemProgram.transfer({
      fromPubkey: authority.publicKey,
      toPubkey: beneficiary.publicKey,
      lamports: 2_000_000,
    });
    await provider.sendAndConfirm(new web3.Transaction().add(transferIx));

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
      totalAmount.toNumber(),
      [],
      undefined,
      TOKEN_PROGRAM_ID,
    );

    beneficiaryTokenAccount = await createAccount(
      provider.connection,
      (authority as any).payer,
      mint,
      beneficiary.publicKey,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID,
    );

    [configPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("vesting-config"),
        authority.publicKey.toBuffer(),
        mint.toBuffer(),
        vestingId.toArrayLike(Buffer, "le", 8),
      ],
      program.programId,
    );

    [vaultPda] = PublicKey.findProgramAddressSync([Buffer.from("vesting-vault"), configPda.toBuffer()], program.programId);

    [schedulePda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vesting-schedule"), configPda.toBuffer(), beneficiary.publicKey.toBuffer()],
      program.programId,
    );
  });

  it("step 2: initialize vesting config", async function () {
    if (!mint || !configPda || !vaultPda) {
      this.skip();
      return;
    }

    await program.methods
      .initializeVestingConfig(vestingId)
      .accountsPartial({
        authority: authority.publicKey,
        mint,
        config: configPda,
        vault: vaultPda,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();
  });

  it("step 3: create beneficiary schedule", async function () {
    if (!beneficiary || !mint || !sourceTokenAccount || !configPda || !vaultPda || !schedulePda) {
      this.skip();
      return;
    }

    const now = Math.floor(Date.now() / 1000);
    startTs = new anchor.BN(now - 100);
    cliffTs = new anchor.BN(now - 50);
    endTs = new anchor.BN(now + 1000);

    await program.methods
      .createSchedule(totalAmount, startTs, cliffTs, endTs)
      .accountsPartial({
        authority: authority.publicKey,
        beneficiary: beneficiary.publicKey,
        config: configPda,
        mint,
        sourceTokenAccount,
        vault: vaultPda,
        schedule: schedulePda,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const vaultAccount = await getAccount(provider.connection, vaultPda, undefined, TOKEN_PROGRAM_ID);
    assert.equal(Number(vaultAccount.amount), totalAmount.toNumber());
  });

  it("step 4: beneficiary claims vested tokens", async function () {
    if (!beneficiary || !configPda || !mint || !schedulePda || !beneficiaryTokenAccount || !vaultPda) {
      this.skip();
      return;
    }

    await program.methods
      .claim()
      .accountsPartial({
        beneficiary: beneficiary.publicKey,
        config: configPda,
        mint,
        schedule: schedulePda,
        beneficiaryTokenAccount,
        vault: vaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([beneficiary])
      .rpc();

    const beneficiaryAccount = await getAccount(provider.connection, beneficiaryTokenAccount, undefined, TOKEN_PROGRAM_ID);
    firstClaimAmount = Number(beneficiaryAccount.amount);

    assert.isAbove(firstClaimAmount, 0);
    assert.isBelow(firstClaimAmount, totalAmount.toNumber());
  });

  it("step 5: authority revokes unvested tokens", async function () {
    if (!mint || !sourceTokenAccount || !configPda || !schedulePda || !vaultPda) {
      this.skip();
      return;
    }

    await program.methods
      .revoke()
      .accountsPartial({
        authority: authority.publicKey,
        config: configPda,
        mint,
        schedule: schedulePda,
        revokeDestination: sourceTokenAccount,
        vault: vaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const schedule = await program.account.vestingSchedule.fetch(schedulePda);
    assert.isAbove(Number(schedule.revokedAt), 0);

    const sourceAccount = await getAccount(provider.connection, sourceTokenAccount, undefined, TOKEN_PROGRAM_ID);
    sourceAmountAfterRevoke = Number(sourceAccount.amount);
    assert.isAbove(sourceAmountAfterRevoke, 0);
  });

  it("step 6: second revoke fails", async function () {
    if (!mint || !sourceTokenAccount || !configPda || !schedulePda || !vaultPda) {
      this.skip();
      return;
    }

    try {
      await program.methods
        .revoke()
        .accountsPartial({
          authority: authority.publicKey,
          config: configPda,
          mint,
          schedule: schedulePda,
          revokeDestination: sourceTokenAccount,
          vault: vaultPda,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();
      assert.fail("second revoke should fail");
    } catch (e) {
      assert.include(`${e}`, "AlreadyRevoked");
    }
  });
});
