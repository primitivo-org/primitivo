import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { TOKEN_PROGRAM_ID, createAccount, createMint, getAccount, mintTo } from "@solana/spl-token";
import { Keypair, PublicKey } from "@solana/web3.js";
import { assert } from "chai";
import { Vault } from "../target/types/vault";

describe("vault integration", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const provider = anchor.getProvider() as anchor.AnchorProvider;
  const program = anchor.workspace.Vault as Program<Vault>;
  const authority = provider.wallet;

  const vaultId = new anchor.BN(1);

  let user: Keypair | undefined;
  let underlyingMint: PublicKey | undefined;
  let configPda: PublicKey | undefined;
  let underlyingVaultPda: PublicKey | undefined;
  let derivativeMintPda: PublicKey | undefined;

  let userUnderlying: PublicKey | undefined;
  let userDerivative: PublicKey | undefined;

  it("step 1: setup user and mint", async () => {
    user = Keypair.generate();

    underlyingMint = await createMint(
      provider.connection,
      (authority as any).payer,
      authority.publicKey,
      null,
      6,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID,
    );

    userUnderlying = await createAccount(
      provider.connection,
      (authority as any).payer,
      underlyingMint,
      user.publicKey,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID,
    );

    await mintTo(
      provider.connection,
      (authority as any).payer,
      underlyingMint,
      userUnderlying,
      authority.publicKey,
      10_000_000,
      [],
      undefined,
      TOKEN_PROGRAM_ID,
    );

    [configPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("vault-config"),
        authority.publicKey.toBuffer(),
        underlyingMint.toBuffer(),
        vaultId.toArrayLike(Buffer, "le", 8),
      ],
      program.programId,
    );

    [underlyingVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault-underlying"), configPda.toBuffer()],
      program.programId,
    );

    [derivativeMintPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault-derivative-mint"), configPda.toBuffer()],
      program.programId,
    );
  });

  it("step 2: initialize vault", async function () {
    if (!underlyingMint || !configPda || !underlyingVaultPda || !derivativeMintPda) {
      this.skip();
      return;
    }

    await program.methods
      .initializeVault(vaultId, 6)
      .accountsPartial({
        authority: authority.publicKey,
        underlyingMint,
        config: configPda,
        underlyingVault: underlyingVaultPda,
        derivativeMint: derivativeMintPda,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    userDerivative = await createAccount(
      provider.connection,
      (authority as any).payer,
      derivativeMintPda,
      user!.publicKey,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID,
    );
  });

  it("step 3: deposit mints derivative", async function () {
    if (!user || !underlyingMint || !configPda || !underlyingVaultPda || !derivativeMintPda || !userUnderlying || !userDerivative) {
      this.skip();
      return;
    }

    const beforeUnderlying = await getAccount(provider.connection, userUnderlying, undefined, TOKEN_PROGRAM_ID);
    const beforeDerivative = await getAccount(provider.connection, userDerivative, undefined, TOKEN_PROGRAM_ID);

    await program.methods
      .deposit(new anchor.BN(1_000_000))
      .accountsPartial({
        user: user.publicKey,
        config: configPda,
        underlyingMint,
        derivativeMint: derivativeMintPda,
        userUnderlyingAccount: userUnderlying,
        userDerivativeAccount: userDerivative,
        underlyingVault: underlyingVaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([user])
      .rpc();

    const afterUnderlying = await getAccount(provider.connection, userUnderlying, undefined, TOKEN_PROGRAM_ID);
    const afterDerivative = await getAccount(provider.connection, userDerivative, undefined, TOKEN_PROGRAM_ID);
    const cfg = await program.account.vaultConfig.fetch(configPda);

    assert.equal(Number(beforeUnderlying.amount) - Number(afterUnderlying.amount), 1_000_000);
    assert.equal(Number(afterDerivative.amount) - Number(beforeDerivative.amount), 1_000_000);
    assert.equal(cfg.underlyingAssets.toNumber(), 1_000_000);
  });

  it("step 4: side profit increases vault underlying assets", async function () {
    if (!underlyingMint || !underlyingVaultPda || !configPda) {
      this.skip();
      return;
    }

    await mintTo(
      provider.connection,
      (authority as any).payer,
      underlyingMint,
      underlyingVaultPda,
      authority.publicKey,
      500_000,
      [],
      undefined,
      TOKEN_PROGRAM_ID,
    );

    const vault = await getAccount(provider.connection, underlyingVaultPda, undefined, TOKEN_PROGRAM_ID);
    assert.equal(Number(vault.amount), 1_500_000);
  });

  it("step 5: redeem returns increased underlying due to profit", async function () {
    if (!user || !underlyingMint || !configPda || !underlyingVaultPda || !derivativeMintPda || !userUnderlying || !userDerivative) {
      this.skip();
      return;
    }

    const beforeUnderlying = await getAccount(provider.connection, userUnderlying, undefined, TOKEN_PROGRAM_ID);
    const beforeDerivative = await getAccount(provider.connection, userDerivative, undefined, TOKEN_PROGRAM_ID);

    await program.methods
      .redeem(new anchor.BN(400_000))
      .accountsPartial({
        user: user.publicKey,
        config: configPda,
        underlyingMint,
        derivativeMint: derivativeMintPda,
        userUnderlyingAccount: userUnderlying,
        userDerivativeAccount: userDerivative,
        underlyingVault: underlyingVaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([user])
      .rpc();

    const afterUnderlying = await getAccount(provider.connection, userUnderlying, undefined, TOKEN_PROGRAM_ID);
    const afterDerivative = await getAccount(provider.connection, userDerivative, undefined, TOKEN_PROGRAM_ID);
    const cfg = await program.account.vaultConfig.fetch(configPda);

    // 400_000 derivative out of 1_000_000 supply against 1_500_000 underlying = 600_000.
    assert.equal(Number(afterUnderlying.amount) - Number(beforeUnderlying.amount), 600_000);
    assert.equal(Number(beforeDerivative.amount) - Number(afterDerivative.amount), 400_000);
    assert.equal(cfg.underlyingAssets.toNumber(), 900_000);
  });
});
