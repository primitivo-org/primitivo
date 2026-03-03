import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import { TOKEN_PROGRAM_ID, createMint } from "@solana/spl-token";
import { Keypair, PublicKey } from "@solana/web3.js";
import { assert } from "chai";
import { Vesting } from "../target/types/vesting";

describe("vesting ownership integration", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const provider = anchor.getProvider() as anchor.AnchorProvider;
  const program = anchor.workspace.Vesting as Program<Vesting>;
  const authority = provider.wallet;

  const vestingId = new anchor.BN(42);
  const ZERO_PUBKEY = new PublicKey("11111111111111111111111111111111");

  let mint: PublicKey | undefined;
  let configPda: PublicKey | undefined;
  let vaultPda: PublicKey | undefined;

  let newOwner: Keypair | undefined;

  it("step 1: initialize vesting config", async () => {
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

    const cfg = await program.account.vestingConfig.fetch(configPda);
    assert.equal(cfg.ownership.owner.toBase58(), authority.publicKey.toBase58());
    assert.equal(cfg.ownership.pendingOwner.toBase58(), ZERO_PUBKEY.toBase58());
    assert.equal(cfg.ownership.pendingExpiresAt.toNumber(), 0);
  });

  it("step 2: owner proposes transfer", async function () {
    if (!configPda) {
      this.skip();
      return;
    }

    newOwner = Keypair.generate();

    const fundingIx = web3.SystemProgram.transfer({
      fromPubkey: authority.publicKey,
      toPubkey: newOwner.publicKey,
      lamports: 2_000_000,
    });
    await provider.sendAndConfirm(new web3.Transaction().add(fundingIx));

    await program.methods
      .proposeOwnershipTransfer(newOwner.publicKey, new anchor.BN(60))
      .accountsPartial({
        owner: authority.publicKey,
        config: configPda,
      })
      .rpc();

    const cfg = await program.account.vestingConfig.fetch(configPda);
    assert.equal(cfg.ownership.pendingOwner.toBase58(), newOwner.publicKey.toBase58());
    assert.isAbove(cfg.ownership.pendingExpiresAt.toNumber(), 0);
  });

  it("step 3: pending owner accepts transfer", async function () {
    if (!configPda || !newOwner) {
      this.skip();
      return;
    }

    await program.methods
      .acceptOwnershipTransfer()
      .accountsPartial({
        pendingOwner: newOwner.publicKey,
        config: configPda,
      })
      .signers([newOwner])
      .rpc();

    const cfg = await program.account.vestingConfig.fetch(configPda);
    assert.equal(cfg.ownership.owner.toBase58(), newOwner.publicKey.toBase58());
    assert.equal(cfg.ownership.pendingOwner.toBase58(), ZERO_PUBKEY.toBase58());
    assert.equal(cfg.ownership.pendingExpiresAt.toNumber(), 0);
  });

  it("step 4: new owner can propose and cancel", async function () {
    if (!configPda || !newOwner) {
      this.skip();
      return;
    }

    const anotherOwner = Keypair.generate();

    await program.methods
      .proposeOwnershipTransfer(anotherOwner.publicKey, new anchor.BN(60))
      .accountsPartial({
        owner: newOwner.publicKey,
        config: configPda,
      })
      .signers([newOwner])
      .rpc();

    await program.methods
      .cancelOwnershipTransfer()
      .accountsPartial({
        owner: newOwner.publicKey,
        config: configPda,
      })
      .signers([newOwner])
      .rpc();

    const cfg = await program.account.vestingConfig.fetch(configPda);
    assert.equal(cfg.ownership.owner.toBase58(), newOwner.publicKey.toBase58());
    assert.equal(cfg.ownership.pendingOwner.toBase58(), ZERO_PUBKEY.toBase58());
    assert.equal(cfg.ownership.pendingExpiresAt.toNumber(), 0);
  });

  it("step 5: expired pending transfer cannot be accepted", async function () {
    if (!configPda || !newOwner) {
      this.skip();
      return;
    }

    const expiringOwner = Keypair.generate();
    const fundingIx = web3.SystemProgram.transfer({
      fromPubkey: authority.publicKey,
      toPubkey: expiringOwner.publicKey,
      lamports: 2_000_000,
    });
    await provider.sendAndConfirm(new web3.Transaction().add(fundingIx));

    await program.methods
      .proposeOwnershipTransfer(expiringOwner.publicKey, new anchor.BN(1))
      .accountsPartial({
        owner: newOwner.publicKey,
        config: configPda,
      })
      .signers([newOwner])
      .rpc();
    
    async function pumpNetwork(connection: web3.Connection, provider: anchor.AnchorProvider, times = 30) {
      for (let i = 0; i < times; i++) {
        const ix = web3.SystemProgram.transfer({
          fromPubkey: provider.wallet.publicKey,
          toPubkey: provider.wallet.publicKey,
          lamports: 0,
        });
        await provider.sendAndConfirm(new web3.Transaction().add(ix));
        await new Promise(r => setTimeout(r, 200));
      }
    }
    await pumpNetwork(provider.connection, provider, 40);

    await new Promise((resolve) => setTimeout(resolve, 2000));

    let failed = false;
    try {
      await program.methods
        .acceptOwnershipTransfer()
        .accountsPartial({
          pendingOwner: expiringOwner.publicKey,
          config: configPda,
        })
        .signers([expiringOwner])
        .rpc();
    } catch (e) {
      failed = true;
    }
    assert.isTrue(failed, "accept should fail after expiry");

    const cfg = await program.account.vestingConfig.fetch(configPda);
    assert.equal(cfg.ownership.owner.toBase58(), newOwner.publicKey.toBase58());

    // Cleanup if pending transfer is still present due non-expiry failure mode.
    if (cfg.ownership.pendingOwner.toBase58() !== ZERO_PUBKEY.toBase58()) {
      await program.methods
        .cancelOwnershipTransfer()
        .accountsPartial({
          owner: newOwner.publicKey,
          config: configPda,
        })
        .signers([newOwner])
        .rpc();
    }
  });
});
