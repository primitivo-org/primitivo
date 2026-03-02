import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import { TOKEN_PROGRAM_ID, createAccount, createMint, getAccount, mintTo } from "@solana/spl-token";
import { Keypair, PublicKey } from "@solana/web3.js";
import { assert } from "chai";
import { Converter } from "../target/types/converter";

describe("converter integration", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const provider = anchor.getProvider() as anchor.AnchorProvider;
  const program = anchor.workspace.Converter as Program<Converter>;
  const authority = provider.wallet;

  const converterId = new anchor.BN(1);

  let user: Keypair | undefined;
  let outsider: Keypair | undefined;

  let fromMint: PublicKey | undefined;
  let toMint: PublicKey | undefined;

  let configPda: PublicKey | undefined;
  let fromVaultPda: PublicKey | undefined;
  let toVaultPda: PublicKey | undefined;

  let userFromToken: PublicKey | undefined;
  let userToToken: PublicKey | undefined;

  it("step 1: setup users, mints, and PDAs", async () => {
    user = Keypair.generate();
    outsider = Keypair.generate();

    for (const kp of [user, outsider]) {
      const tx = new web3.Transaction().add(
        web3.SystemProgram.transfer({
          fromPubkey: authority.publicKey,
          toPubkey: kp.publicKey,
          lamports: 2_000_000,
        }),
      );
      await provider.sendAndConfirm(tx);
    }

    fromMint = await createMint(
      provider.connection,
      (authority as any).payer,
      authority.publicKey,
      null,
      6,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID,
    );

    toMint = await createMint(
      provider.connection,
      (authority as any).payer,
      authority.publicKey,
      null,
      6,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID,
    );

    userFromToken = await createAccount(
      provider.connection,
      (authority as any).payer,
      fromMint,
      user.publicKey,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID,
    );

    userToToken = await createAccount(
      provider.connection,
      (authority as any).payer,
      toMint,
      user.publicKey,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID,
    );

    await mintTo(
      provider.connection,
      (authority as any).payer,
      fromMint,
      userFromToken,
      authority.publicKey,
      1_000_000,
      [],
      undefined,
      TOKEN_PROGRAM_ID,
    );

    [configPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("converter-config"),
        authority.publicKey.toBuffer(),
        fromMint.toBuffer(),
        toMint.toBuffer(),
        converterId.toArrayLike(Buffer, "le", 8),
      ],
      program.programId,
    );

    [fromVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("converter-from-vault"), configPda.toBuffer()],
      program.programId,
    );

    [toVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("converter-to-vault"), configPda.toBuffer()],
      program.programId,
    );
  });

  it("step 2: initialize converter", async function () {
    if (!fromMint || !toMint || !configPda || !fromVaultPda || !toVaultPda) {
      this.skip();
      return;
    }

    await program.methods
      .initializeConverter(converterId, new anchor.BN(2), new anchor.BN(1))
      .accountsPartial({
        authority: authority.publicKey,
        fromMint,
        toMint,
        config: configPda,
        fromVault: fromVaultPda,
        toVault: toVaultPda,
        systemProgram: web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();
  });

  it("step 3: fund output vault", async function () {
    if (!toMint || !toVaultPda) {
      this.skip();
      return;
    }

    await mintTo(
      provider.connection,
      (authority as any).payer,
      toMint,
      toVaultPda,
      authority.publicKey,
      10_000_000,
      [],
      undefined,
      TOKEN_PROGRAM_ID,
    );

    const vault = await getAccount(provider.connection, toVaultPda, undefined, TOKEN_PROGRAM_ID);
    assert.equal(Number(vault.amount), 10_000_000);
  });

  it("step 4: swap succeeds with minimum_received", async function () {
    if (!user || !configPda || !fromMint || !toMint || !userFromToken || !userToToken || !fromVaultPda || !toVaultPda) {
      this.skip();
      return;
    }

    const beforeFrom = await getAccount(provider.connection, userFromToken, undefined, TOKEN_PROGRAM_ID);
    const beforeTo = await getAccount(provider.connection, userToToken, undefined, TOKEN_PROGRAM_ID);

    await program.methods
      .swap(new anchor.BN(1_000), new anchor.BN(1_900))
      .accountsPartial({
        user: user.publicKey,
        config: configPda,
        fromMint,
        toMint,
        userFromAccount: userFromToken,
        userToAccount: userToToken,
        fromVault: fromVaultPda,
        toVault: toVaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([user])
      .rpc();

    const afterFrom = await getAccount(provider.connection, userFromToken, undefined, TOKEN_PROGRAM_ID);
    const afterTo = await getAccount(provider.connection, userToToken, undefined, TOKEN_PROGRAM_ID);

    assert.equal(Number(beforeFrom.amount) - Number(afterFrom.amount), 1_000);
    assert.equal(Number(afterTo.amount) - Number(beforeTo.amount), 2_000);
  });

  it("step 5: owner can update rate", async function () {
    if (!configPda) {
      this.skip();
      return;
    }

    await program.methods
      .updateRate(new anchor.BN(1), new anchor.BN(1))
      .accountsPartial({
        owner: authority.publicKey,
        config: configPda,
      })
      .rpc();

    const cfg = await program.account.converterConfig.fetch(configPda);
    assert.equal(cfg.rateNumerator.toNumber(), 1);
    assert.equal(cfg.rateDenominator.toNumber(), 1);
  });

  it("step 6: non-owner update fails", async function () {
    if (!outsider || !configPda) {
      this.skip();
      return;
    }

    try {
      await program.methods
        .updateRate(new anchor.BN(3), new anchor.BN(2))
        .accountsPartial({
          owner: outsider.publicKey,
          config: configPda,
        })
        .signers([outsider])
        .rpc();
      assert.fail("non-owner update should fail");
    } catch (e) {
      assert.include(`${e}`, "NotOwner");
    }
  });

  it("step 7: swap fails when minimum_received too high", async function () {
    if (!user || !configPda || !fromMint || !toMint || !userFromToken || !userToToken || !fromVaultPda || !toVaultPda) {
      this.skip();
      return;
    }

    try {
      await program.methods
        .swap(new anchor.BN(1_000), new anchor.BN(1_500))
        .accountsPartial({
          user: user.publicKey,
          config: configPda,
          fromMint,
          toMint,
          userFromAccount: userFromToken,
          userToAccount: userToToken,
          fromVault: fromVaultPda,
          toVault: toVaultPda,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([user])
        .rpc();
      assert.fail("slippage protection should fail");
    } catch (e) {
      assert.include(`${e}`, "SlippageExceeded");
    }
  });
});
