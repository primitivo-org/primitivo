<p align="center">
  <img src="./primitivo_logo.svg" alt="primitivo logo" width="180" />
</p>

# Primitivo

Anchor workspace for Solana primitives

## Workspace Layout

- `programs/merke-airdrop`: on-chain Anchor program entrypoints/accounts.
- `programs/vesting`: on-chain vesting program (cliff + linear release + revoke).
- `programs/converter`: on-chain SPL token converter with owner-managed exchange rate.
- `programs/vault`: on-chain vault for underlying SPL token and derivative token.
- `crates/macro`: shared ownership and pausable helpers.
- `crates/airdrop-merkle`: shared Merkle airdrop logic and account types.
- `crates/vesting`: shared vesting logic.
- `crates/converter`: shared token converter logic.
- `crates/vault`: shared vault logic and account types.
- `utils/merkle-tree-generator`: Bun TypeScript tool for `root` and `proof` commands.


## Program ID Env Overrides

Each shared crate with program-specific state generates its program ID from env vars in its own `build.rs`.

Supported env vars:

- `PRIMITIVO_MERKLE_AIRDROP_ID`
- `PRIMITIVO_VESTING_ID`
- `PRIMITIVO_CONVERTER_ID`
- `PRIMITIVO_VAULT_ID`

You can override IDs per build:

```bash
PRIMITIVO_MERKLE_AIRDROP_ID=<YOUR_AIRDROP_PROGRAM_ID> \
PRIMITIVO_VESTING_ID=<YOUR_VESTING_PROGRAM_ID> \
anchor build
```

Local defaults are set in `.cargo/config.toml` for this repo.

## Ownership Helper

`crates/macro/src/ownership.rs` provides reusable ownership state:

- `owner`
- `pending_owner`
- `pending_expires_at`

It is now embedded in on-chain state:

- `Distributor.ownership` in `merke_airdrop`
- `VestingConfig.ownership` in `vesting`

Current usage in both programs enforces authority via `ownership.owner`.

Helper methods:

- `require_owner(signer)`
- `propose_transfer(signer, new_owner, now_ts, accept_window_secs)`
- `accept_transfer(signer, now_ts)`
- `cancel_transfer(signer)`

Ownership macros for programs:

- `generate_ownership_transfer_accounts!`
- `generate_ownership_transfer_handlers!`

Pausable macros for programs:

- `generate_pausable_accounts!`
- `generate_pausable_handlers!`
- `require_not_paused!(ctx, state_account)`

Detailed usage:

1. Ensure your state account has an `ownership: Ownership` field.
2. Generate account context structs near the bottom of your program file:

```rust
primitivo_macro::generate_ownership_transfer_accounts!(
    state_ty = Distributor,
    state_account = distributor,
    propose_ctx = ProposeOwnershipTransfer,
    accept_ctx = AcceptOwnershipTransfer,
    cancel_ctx = CancelOwnershipTransfer
);
```

3. Generate instruction handlers inside `#[program] mod ...`:

```rust
primitivo_macro::generate_ownership_transfer_handlers!(
    propose_fn = propose_ownership_transfer,
    accept_fn = accept_ownership_transfer,
    cancel_fn = cancel_ownership_transfer,
    propose_ctx = ProposeOwnershipTransfer,
    accept_ctx = AcceptOwnershipTransfer,
    cancel_ctx = CancelOwnershipTransfer,
    state_account = distributor
);
```

Macro arguments:

- `state_ty`: Anchor account struct type containing `ownership`.
- `state_account`: account field name in generated contexts.
- `propose_ctx` / `accept_ctx` / `cancel_ctx`: generated `Accounts` struct names.
- `propose_fn` / `accept_fn` / `cancel_fn`: generated instruction function names.

Generated behavior:

- `propose`: current owner sets `pending_owner` + expiry window.
- `accept`: pending owner accepts before expiry and becomes `owner`.
- `cancel`: current owner clears pending transfer.

Program instructions using this helper:

- `merke_airdrop`: `propose_ownership_transfer`, `accept_ownership_transfer`, `cancel_ownership_transfer`
- `vesting`: `propose_ownership_transfer`, `accept_ownership_transfer`, `cancel_ownership_transfer`

## Build / Test

```bash
anchor build
anchor test
```

## Vesting Program

`vesting` supports:

- configurable SPL mint per vesting config
- per-user vesting schedule
- cliff timestamp
- linear release between `start_ts` and `end_ts`
- authority revoke of unvested amount

Main instructions:

- `initialize_vesting_config(id)`
- `create_schedule(total_amount, start_ts, cliff_ts, end_ts)`
- `claim()`
- `revoke()`
- `pause()` / `unpause()`

## Converter Program

`converter` supports:

- configurable `from_mint` / `to_mint`
- owner-only exchange rate updates (`rate_numerator` / `rate_denominator`)
- token swap with slippage guard via `minimum_received`

Main instructions:

- `initialize_converter(id, rate_numerator, rate_denominator)`
- `update_rate(rate_numerator, rate_denominator)` (owner only)
- `swap(amount_in, minimum_received)`

## Vault Program

`vault` supports:

- one underlying SPL token configured at init
- derivative token mint controlled by vault program
- deposit underlying -> mint derivative at current rate
- redeem derivative -> return underlying at current rate
- tracks `underlying_assets` in config state

Rate model:

- `deposit`: `derivative_out = amount_in * supply / underlying_assets` (or `1:1` when empty)
- `redeem`: `underlying_out = derivative_in * underlying_assets / supply`

Main instructions:

- `initialize_vault(id, derivative_decimals)`
- `deposit(underlying_amount)`
- `redeem(derivative_amount)`

## Merkle Data Generator (Bun)

Airdrop file format is one line per user:

```txt
<address> <amount>
```

Example:

```txt
BNGRxgaJ9TBB2RScFDhZCxvHoXqtpJnG6BMTZjfeZETj 100
55XMjVxUhLqErahWDJ1gikHhxAufWivYkTvs4DcRh7ry 200
AXruuuQaXDZQd7t4pJfFTp1zDpYiDg6QMCBfk9UTHQoN 300
```

Commands:

```bash
cd utils/merkle-tree-generator
bun install
bun run root --input ./addresses.txt
bun run proof --input ./addresses.txt --address <WALLET>
```

`proof` output includes `index`, `amount`, and `proof` for on-chain claim.
