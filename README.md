# Solana Airdrop (Anchor + Merkle)

Minimal setup for an SPL-token merkle airdrop where leaves are `(address, amount)`.

## Prerequisites

- Solana CLI + local validator
- Anchor CLI
- Node.js + npm
- Bun (for `merkle-tree-generator`)

## Build and Deploy

```bash
# 1) build program
anchor build

# 2) start local validator (in another terminal)
solana-test-validator --reset

# 3) deploy to local validator
ANCHOR_PROVIDER_URL=http://127.0.0.1:8899 \
ANCHOR_WALLET=$HOME/.config/solana/id.json \
anchor deploy
```

## Merkle Tree Generator

Input file format (`merkle-tree-generator/addresses.txt`):

```txt
<address> <amount>
<address> <amount>
```


Run generator:

```bash
cd merkle-tree-generator
# if bun is on PATH
bun install
bun run root --input ./addresses.txt
bun run proof --input ./addresses.txt --address <WALLET>

`proof` output includes the exact `amount` for the address; pass both `amount` and `proof` to on-chain `claim`.

## Tests

```bash

# merkle generator unit tests
cd merkle-tree-generator
bun run test

# integration test (requires running local validator + deployed program)
cd ..
anchor test
```
