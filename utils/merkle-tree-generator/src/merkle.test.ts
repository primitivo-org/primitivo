import { expect, test } from "bun:test";
import { AirdropEntry, getProof, getRoot, hashLeaf, hashPair, toHex } from "./merkle";

const ENTRIES: AirdropEntry[] = [
  { address: "BNGRxgaJ9TBB2RScFDhZCxvHoXqtpJnG6BMTZjfeZETj", amount: 100n },
  { address: "55XMjVxUhLqErahWDJ1gikHhxAufWivYkTvs4DcRh7ry", amount: 200n },
  { address: "AXruuuQaXDZQd7t4pJfFTp1zDpYiDg6QMCBfk9UTHQoN", amount: 300n },
];

function applyProof(index: number, address: string, amount: bigint, proof: Buffer[]): Buffer {
  let node = hashLeaf(index, address, amount);
  for (const sibling of proof) {
    node = hashPair(node, sibling);
  }
  return node;
}

test("getRoot returns deterministic root for known airdrop list", () => {
  const root = getRoot(ENTRIES);
  expect(toHex(root)).toBe("0xf2c573e20c4257484e00b8bf829aba233cf1251d4f097549b2a9eab97eb63f3e");
});

test("getProof reconstructs the same root", () => {
  const root = getRoot(ENTRIES);

  ENTRIES.forEach((entry, index) => {
    const { amount, proof } = getProof(ENTRIES, entry.address);
    const recomputed = applyProof(index, entry.address, amount, proof);
    expect(recomputed.equals(root)).toBeTrue();
  });
});

test("getProof throws for address not in list", () => {
  expect(() => getProof(ENTRIES, "11111111111111111111111111111111")).toThrow("Target address is not in the list");
});
