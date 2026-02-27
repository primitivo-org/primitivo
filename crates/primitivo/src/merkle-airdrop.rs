use solana_program::hash::hashv;
use solana_program::pubkey::Pubkey;

pub fn hash_leaf(index: u32, recipient: &Pubkey, amount: u64) -> [u8; 32] {
    let index_bytes = index.to_le_bytes();
    let amount_bytes = amount.to_le_bytes();

    hashv(&[b"merkle_airdrop", &index_bytes, recipient.as_ref(), &amount_bytes]).to_bytes()
}

pub fn hash_pair(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let (left, right) = if a <= b { (a, b) } else { (b, a) };
    hashv(&[left, right]).to_bytes()
}

pub fn verify_proof(leaf: [u8; 32], proof: &[[u8; 32]], root: [u8; 32]) -> bool {
    let mut computed = leaf;

    for sibling in proof {
        computed = hash_pair(&computed, sibling);
    }

    computed == root
}

pub fn bitmap_len(max_claims: u32) -> usize {
    (max_claims as usize).div_ceil(8)
}

pub fn is_claimed(bitmap: &[u8], index: u32) -> bool {
    let byte_index = (index / 8) as usize;
    let bit_mask = 1u8 << (index % 8);
    bitmap
        .get(byte_index)
        .map(|byte| (byte & bit_mask) != 0)
        .unwrap_or(false)
}

pub fn set_claimed(bitmap: &mut [u8], index: u32) -> bool {
    let byte_index = (index / 8) as usize;
    let bit_mask = 1u8 << (index % 8);

    if let Some(byte) = bitmap.get_mut(byte_index) {
        *byte |= bit_mask;
        true
    } else {
        false
    }
}
