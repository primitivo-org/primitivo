import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { PublicKey } from "@solana/web3.js";

const LEAF_PREFIX = Buffer.from("merkle_airdrop", "utf8");

export type AirdropEntry = {
  address: string;
  amount: bigint;
};

function sha256(data: Buffer): Buffer {
  return createHash("sha256").update(data).digest();
}

function amountToLeBytes(amount: bigint): Buffer {
  if (amount <= 0n) {
    throw new Error("Amount must be greater than 0");
  }

  const max = (1n << 64n) - 1n;
  if (amount > max) {
    throw new Error("Amount must fit in u64");
  }

  const out = Buffer.alloc(8);
  out.writeBigUInt64LE(amount);
  return out;
}

export function hashLeaf(address: string, amount: bigint): Buffer {
  const pubkey = new PublicKey(address);
  return sha256(Buffer.concat([LEAF_PREFIX, pubkey.toBuffer(), amountToLeBytes(amount)]));
}

export function hashPair(a: Buffer, b: Buffer): Buffer {
  const [left, right] = Buffer.compare(a, b) <= 0 ? [a, b] : [b, a];
  return sha256(Buffer.concat([left, right]));
}

function parseEntriesFromJson(raw: string): AirdropEntry[] {
  const parsed = JSON.parse(raw);
  if (!Array.isArray(parsed)) {
    throw new Error("JSON input must be an array");
  }

  const entries: AirdropEntry[] = [];
  for (const entry of parsed) {
    if (
      typeof entry !== "object" ||
      entry === null ||
      typeof entry.address !== "string" ||
      (typeof entry.amount !== "string" && typeof entry.amount !== "number")
    ) {
      throw new Error("JSON entries must be objects: { address: string, amount: string|number }");
    }

    entries.push({
      address: entry.address,
      amount: BigInt(entry.amount),
    });
  }

  return entries;
}

function parseEntriesFromPlainText(raw: string): AirdropEntry[] {
  const lines = raw
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);

  const entries: AirdropEntry[] = [];
  for (const line of lines) {
    const parts = line.split(/[\s,]+/).filter(Boolean);
    if (parts.length !== 2) {
      throw new Error(`Invalid line format: "${line}". Expected: <address> <amount>`);
    }

    entries.push({
      address: parts[0],
      amount: BigInt(parts[1]),
    });
  }

  return entries;
}

function ensureValidEntries(entries: AirdropEntry[]): AirdropEntry[] {
  if (entries.length === 0) {
    return [];
  }

  for (const entry of entries) {
    new PublicKey(entry.address);
    amountToLeBytes(entry.amount);
  }

  return entries;
}

export function loadEntries(filePath: string): AirdropEntry[] {
  const raw = readFileSync(resolve(filePath), "utf8").trim();
  if (!raw) {
    return [];
  }

  const entries = raw.startsWith("[") ? parseEntriesFromJson(raw) : parseEntriesFromPlainText(raw);
  return ensureValidEntries(entries);
}

export function buildTree(leaves: Buffer[]): Buffer[][] {
  if (leaves.length === 0) {
    throw new Error("Airdrop list is empty");
  }

  const levels: Buffer[][] = [leaves];
  while (levels[levels.length - 1].length > 1) {
    const current = levels[levels.length - 1];
    const next: Buffer[] = [];

    for (let i = 0; i < current.length; i += 2) {
      const left = current[i];
      const right = current[i + 1] ?? current[i];
      next.push(hashPair(left, right));
    }

    levels.push(next);
  }

  return levels;
}

export function getRoot(entries: AirdropEntry[]): Buffer {
  const leaves = entries.map((entry) => hashLeaf(entry.address, entry.amount));
  const tree = buildTree(leaves);
  return tree[tree.length - 1][0];
}

export function getProof(
  entries: AirdropEntry[],
  targetAddress: string,
): { index: number; amount: bigint; proof: Buffer[] } {
  const matches = entries
    .map((entry, index) => ({ ...entry, index }))
    .filter((entry) => entry.address === targetAddress);

  if (matches.length === 0) {
    throw new Error("Target address is not in the list");
  }
  if (matches.length > 1) {
    throw new Error("Target address appears multiple times; use unique addresses");
  }

  const target = matches[0];
  const leaves = entries.map((entry) => hashLeaf(entry.address, entry.amount));
  const tree = buildTree(leaves);
  const proof: Buffer[] = [];

  let position = target.index;
  for (let level = 0; level < tree.length - 1; level += 1) {
    const nodes = tree[level];
    const siblingIndex = position % 2 === 0 ? position + 1 : position - 1;
    proof.push(nodes[siblingIndex] ?? nodes[position]);
    position = Math.floor(position / 2);
  }

  return { index: target.index, amount: target.amount, proof };
}

export function toHex(buf: Buffer): string {
  return `0x${buf.toString("hex")}`;
}
