# primitivo

Anchor workspace for Solana primitives, currently including a Merkle SPL-token airdrop program and shared Rust crate modules.

## Workspace Layout

- `programs/merke-airdrop`: on-chain Anchor program entrypoints/accounts.
- `programs/vesting`: on-chain vesting program (cliff + linear release + revoke).
- `crates/primitivo`: reusable Rust module crate (state, handlers, merkle/bitmap logic).
- `utils/merkle-tree-generator`: Bun TypeScript tool for `root` and `proof` commands.
- `scripts`: TypeScript scripts for distributor initialization and claims.

## Program ID Env Overrides

`crates/primitivo/build.rs` generates program-id files from env vars.

Supported env vars:

- `PRIMITIVO_MERKLE_AIRDROP_ID`
- `PRIMITIVO_VESTING_ID`

You can override IDs per build:

```bash
PRIMITIVO_MERKLE_AIRDROP_ID=<YOUR_AIRDROP_PROGRAM_ID> \
PRIMITIVO_VESTING_ID=<YOUR_VESTING_PROGRAM_ID> \
anchor build
```

Local defaults are set in `.cargo/config.toml` for this repo.

## Ownership Helper

`crates/primitivo/src/ownership.rs` provides reusable ownership state:

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

## Initialize Distributor

With mint creation:

```bash
anchor run init -- \
  --program-id <PROGRAM_ID> \
  --airdrop-file ./utils/merkle-tree-generator/addresses.txt \
  --id 1 \
  --create-mint \
  --decimals 6
```

With existing mint and source token account:

```bash
anchor run init -- \
  --program-id <PROGRAM_ID> \
  --airdrop-file ./utils/merkle-tree-generator/addresses.txt \
  --id 1 \
  --mint <MINT_PUBKEY> \
  --source-token-account <SOURCE_TOKEN_ACCOUNT>
```

Optional:

- `--funding-amount <u64>` (default is sum of list amounts)

## Claim

```bash
anchor run claim -- \
  --program-id <PROGRAM_ID> \
  --distributor <DISTRIBUTOR_PDA> \
  --proof-file ./proof.json \
  --create-token-account
```

Or provide an existing token account:

```bash
anchor run claim -- \
  --program-id <PROGRAM_ID> \
  --distributor <DISTRIBUTOR_PDA> \
  --proof-file ./proof.json \
  --claimant-token-account <TOKEN_ACCOUNT>
```

Claim status uses on-chain bitmap PDA: `['bitmap', distributor]`, keyed by Merkle `index`.
