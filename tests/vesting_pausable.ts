import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import { TOKEN_PROGRAM_ID, createAccount, createMint, getAccount, mintTo } from "@solana/spl-token";
import { Keypair, PublicKey } from "@solana/web3.js";
import { assert } from "chai";
import { Vesting } from "../target/types/vesting";

describe("vesting pausable integration", () => {
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

  const vestingId = new anchor.BN(77);
  const totalAmount = new anchor.BN(1_000_000);

  it("step 1: setup and initialize config", async () => {
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

  it("step 2: owner pauses program", async function () {
    if (!configPda) {
      this.skip();
      return;
    }

    await program.methods
      .pause()
      .accountsPartial({
        owner: authority.publicKey,
        config: configPda,
      })
      .rpc();

    const cfg = await program.account.vestingConfig.fetch(configPda);
    assert.equal(cfg.pausable.paused, true);
  });

  it("step 3: create schedule fails while paused", async function () {
    if (!beneficiary || !mint || !sourceTokenAccount || !configPda || !vaultPda || !schedulePda) {
      this.skip();
      return;
    }

    const now = Math.floor(Date.now() / 1000);
    const startTs = new anchor.BN(now - 100);
    const cliffTs = new anchor.BN(now - 50);
    const endTs = new anchor.BN(now + 1000);

    try {
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
      assert.fail("create schedule should fail when paused");
    } catch (e) {
      assert.include(`${e}`, "ProgramPaused");
    }
  });

  it("step 4: owner unpauses", async function () {
    if (!configPda) {
      this.skip();
      return;
    }

    await program.methods
      .unpause()
      .accountsPartial({
        owner: authority.publicKey,
        config: configPda,
      })
      .rpc();

    const cfg = await program.account.vestingConfig.fetch(configPda);
    assert.equal(cfg.pausable.paused, false);
  });

  it("step 5: create schedule succeeds after unpause", async function () {
    if (!beneficiary || !mint || !sourceTokenAccount || !configPda || !vaultPda || !schedulePda) {
      this.skip();
      return;
    }

    const now = Math.floor(Date.now() / 1000);
    const startTs = new anchor.BN(now - 100);
    const cliffTs = new anchor.BN(now - 50);
    const endTs = new anchor.BN(now + 1000);

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

    const vault = await getAccount(provider.connection, vaultPda, undefined, TOKEN_PROGRAM_ID);
    assert.equal(Number(vault.amount), totalAmount.toNumber());
  });
});
