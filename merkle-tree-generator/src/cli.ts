#!/usr/bin/env bun
import { Command } from "commander";
import { getProof, getRoot, loadEntries, toHex } from "./merkle";

const program = new Command();
program.name("merkle-tree-generator").description("Generate merkle root and proofs for Solana addresses");

program
  .command("root")
  .requiredOption("-i, --input <file>", "Path to airdrop list: <address> <amount> per line or JSON")
  .action((opts) => {
    const entries = loadEntries(opts.input);
    const root = getRoot(entries);
    console.log(JSON.stringify({ count: entries.length, merkleRoot: toHex(root) }, null, 2));
  });

program
  .command("proof")
  .requiredOption("-i, --input <file>", "Path to airdrop list")
  .requiredOption("-a, --address <address>", "Address to generate proof for")
  .action((opts) => {
    const entries = loadEntries(opts.input);
    const { index, amount, proof } = getProof(entries, opts.address);
    console.log(
      JSON.stringify(
        {
          address: opts.address,
          amount: amount.toString(),
          index,
          proof: proof.map(toHex),
        },
        null,
        2,
      ),
    );
  });

program.parse();
