# Solana Airdrop (Anchor + Merkle)

Merkle airdrop program where each leaf is `(address, amount)`.

## Deploy

```bash
# build
anchor build

# start local validator (separate terminal)
solana-test-validator --reset

# deploy
ANCHOR_PROVIDER_URL=http://127.0.0.1:8899 \
ANCHOR_WALLET=$HOME/.config/solana/id.json \
anchor deploy
```

## Generate Merkle Data

Airdrop file format (`address amount` per line):

```txt
BNGRxgaJ9TBB2RScFDhZCxvHoXqtpJnG6BMTZjfeZETj 100
55XMjVxUhLqErahWDJ1gikHhxAufWivYkTvs4DcRh7ry 200
AXruuuQaXDZQd7t4pJfFTp1zDpYiDg6QMCBfk9UTHQoN 300
```

```bash
cd merkle-tree-generator
bun install
bun run root --input ./addresses.txt
bun run proof --input ./addresses.txt --address <WALLET>
```

`proof` output includes `index`, `amount`, and `proof`. On claim, pass all three.

## Initialize Distributor Script

After deploy, initialize distributor using the generated root and funding.

### Option A: create a new SPL mint (optional)

```bash
npm run init:distributor -- \
  --program-id <PROGRAM_ID> \
  --airdrop-file ./merkle-tree-generator/addresses.txt \
  --id 1 \
  --create-mint \
  --decimals 6
```

### Option B: use existing mint + source token account

```bash
npm run init:distributor -- \
  --program-id <PROGRAM_ID> \
  --airdrop-file ./merkle-tree-generator/addresses.txt \
  --id 1 \
  --mint <MINT_PUBKEY> \
  --source-token-account <SOURCE_TOKEN_ACCOUNT>
```

Optional flag:

- `--funding-amount <u64>` to override default funding (default is sum of all amounts in the list).

The script prints distributor PDA, vault PDA, mint, tx signature, and merkle root.

## Claim Script

Use proof output from `bun run proof ...`:

```bash
npm run claim -- \
  --program-id <PROGRAM_ID> \
  --distributor <DISTRIBUTOR_PDA> \
  --proof-file ./proof.json \
  --create-token-account
```

Or provide an existing token account:

```bash
npm run claim -- \
  --program-id <PROGRAM_ID> \
  --distributor <DISTRIBUTOR_PDA> \
  --proof-file ./proof.json \
  --claimant-token-account <TOKEN_ACCOUNT>
```

With Anchor alias:

```bash
anchor run claim -- \
  --program-id <PROGRAM_ID> \
  --distributor <DISTRIBUTOR_PDA> \
  --proof-file ./proof.json \
  --create-token-account
```

Claim status is tracked on-chain in a bitmap PDA (`[\"bitmap\", distributor]`) using the merkle `index`.
