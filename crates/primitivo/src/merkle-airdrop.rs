use anchor_lang::prelude::*;
use solana_sha256_hasher::hashv;
use crate::Ownership;

include!(concat!(
    env!("OUT_DIR"),
    "/primitivo_merkle_airdrop_program_id.rs"
));

#[account]
#[derive(InitSpace)]
pub struct Distributor {
    pub ownership: Ownership,
    pub seed_authority: Pubkey,
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub merkle_root: [u8; 32],
    pub id: u64,
    pub max_claims: u32,
    pub claimed_amount: u64,
    pub bump: u8,
    pub vault_bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct ClaimBitmap {
    pub distributor: Pubkey,
    pub max_claims: u32,
    #[max_len(0)]
    pub bitmap: Vec<u8>,
    pub bump: u8,
}

impl ClaimBitmap {
    pub fn space(max_claims: u32) -> usize {
        Self::INIT_SPACE + bitmap_len(max_claims)
    }

    pub fn is_claimed(&self, index: u32) -> bool {
        is_claimed(&self.bitmap, index)
    }

    pub fn set_claimed(&mut self, index: u32) -> Result<()> {
        if set_claimed(&mut self.bitmap, index) {
            Ok(())
        } else {
            err!(AirdropError::InvalidClaimIndex)
        }
    }
}

#[error_code]
pub enum AirdropError {
    #[msg("Proof is invalid for this wallet")]
    InvalidProof,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("Source token account owner must be the authority")]
    InvalidSourceOwner,
    #[msg("Unexpected token mint")]
    InvalidMint,
    #[msg("Unexpected vault account")]
    InvalidVault,
    #[msg("Unexpected claimant token account")]
    InvalidRecipientAccount,
    #[msg("Claim amount must be greater than 0")]
    InvalidClaimAmount,
    #[msg("This index has already claimed")]
    AlreadyClaimed,
    #[msg("Claim index out of range")]
    InvalidClaimIndex,
    #[msg("max_claims must be greater than 0")]
    InvalidMaxClaims,
    #[msg("Invalid bitmap account")]
    InvalidBitmap,
}

pub fn initialize_distributor_handler(
    distributor_key: Pubkey,
    distributor: &mut Distributor,
    claim_bitmap: &mut ClaimBitmap,
    authority: Pubkey,
    mint: Pubkey,
    vault: Pubkey,
    id: u64,
    merkle_root: [u8; 32],
    max_claims: u32,
    distributor_bump: u8,
    vault_bump: u8,
    bitmap_bump: u8,
) -> Result<()> {
    require!(max_claims > 0, AirdropError::InvalidMaxClaims);

    distributor.ownership = Ownership::new(authority);
    distributor.seed_authority = authority;
    distributor.mint = mint;
    distributor.vault = vault;
    distributor.merkle_root = merkle_root;
    distributor.id = id;
    distributor.max_claims = max_claims;
    distributor.claimed_amount = 0;
    distributor.bump = distributor_bump;
    distributor.vault_bump = vault_bump;

    claim_bitmap.distributor = distributor_key;
    claim_bitmap.max_claims = max_claims;
    claim_bitmap.bitmap = vec![0u8; bitmap_len(max_claims)];
    claim_bitmap.bump = bitmap_bump;

    Ok(())
}

pub fn claim_handler(
    distributor: &mut Distributor,
    claim_bitmap: &mut ClaimBitmap,
    claimant: Pubkey,
    index: u32,
    amount: u64,
    proof: &[[u8; 32]],
) -> Result<()> {
    require!(amount > 0, AirdropError::InvalidClaimAmount);
    require!(
        index < distributor.max_claims,
        AirdropError::InvalidClaimIndex
    );

    let leaf = hash_leaf(index, &claimant, amount);
    require!(
        verify_proof(leaf, proof, distributor.merkle_root),
        AirdropError::InvalidProof
    );

    require!(
        !claim_bitmap.is_claimed(index),
        AirdropError::AlreadyClaimed
    );
    claim_bitmap.set_claimed(index)?;

    distributor.claimed_amount = distributor
        .claimed_amount
        .checked_add(amount)
        .ok_or(AirdropError::ArithmeticOverflow)?;

    Ok(())
}

pub fn hash_leaf(index: u32, recipient: &Pubkey, amount: u64) -> [u8; 32] {
    let index_bytes = index.to_le_bytes();
    let amount_bytes = amount.to_le_bytes();
    hashv(&[
        b"merkle_airdrop".as_ref(),
        index_bytes.as_ref(),
        recipient.as_ref(),
        amount_bytes.as_ref(),
    ])
    .to_bytes()
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
