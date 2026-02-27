// Stops Rust Analyzer complaining about missing configs
// See https://solana.stackexchange.com/questions/17777
#![allow(unexpected_cfgs)]
// Fix warning: use of deprecated method `anchor_lang::prelude::AccountInfo::<'a>::realloc`: Use AccountInfo::resize() instead
// See https://solana.stackexchange.com/questions/22979
#![allow(deprecated)]

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use primitivo::{bitmap_len, hash_leaf, is_claimed, set_claimed, verify_proof};

declare_id!("Dpjs4ihZc6T9Y6mBfgDcmRavoFysLRDpdW5fezbxGZ33");

#[program]
pub mod merke_airdrop {
    use super::*;

    pub fn initialize_distributor(
        ctx: Context<InitializeDistributor>,
        id: u64,
        merkle_root: [u8; 32],
        max_claims: u32,
        total_funding_amount: u64,
    ) -> Result<()> {
        require!(max_claims > 0, AirdropError::InvalidMaxClaims);

        let distributor = &mut ctx.accounts.distributor;
        distributor.authority = ctx.accounts.authority.key();
        distributor.mint = ctx.accounts.mint.key();
        distributor.vault = ctx.accounts.vault.key();
        distributor.merkle_root = merkle_root;
        distributor.id = id;
        distributor.max_claims = max_claims;
        distributor.claimed_amount = 0;
        distributor.bump = ctx.bumps.distributor;
        distributor.vault_bump = ctx.bumps.vault;

        let bitmap = &mut ctx.accounts.claim_bitmap;
        bitmap.distributor = distributor.key();
        bitmap.max_claims = max_claims;
        bitmap.bitmap = vec![0u8; bitmap_len(max_claims)];
        bitmap.bump = ctx.bumps.claim_bitmap;

        if total_funding_amount > 0 {
            let cpi_accounts = Transfer {
                from: ctx.accounts.source_token_account.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.authority.to_account_info(),
            };
            let cpi_ctx =
                CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
            token::transfer(cpi_ctx, total_funding_amount)?;
        }

        Ok(())
    }

    pub fn claim(ctx: Context<Claim>, index: u32, amount: u64, proof: Vec<[u8; 32]>) -> Result<()> {
        require!(amount > 0, AirdropError::InvalidClaimAmount);

        let distributor = &mut ctx.accounts.distributor;
        require!(
            index < distributor.max_claims,
            AirdropError::InvalidClaimIndex
        );

        let claimant = ctx.accounts.claimant.key();

        let leaf = hash_leaf(index, &claimant, amount);
        require!(
            verify_proof(leaf, &proof, distributor.merkle_root),
            AirdropError::InvalidProof
        );

        let bitmap = &mut ctx.accounts.claim_bitmap;
        require!(!bitmap.is_claimed(index), AirdropError::AlreadyClaimed);
        bitmap.set_claimed(index)?;

        distributor.claimed_amount = distributor
            .claimed_amount
            .checked_add(amount)
            .ok_or(AirdropError::ArithmeticOverflow)?;

        let id_bytes = distributor.id.to_le_bytes();
        let signer_seeds: &[&[u8]] = &[
            b"distributor",
            distributor.authority.as_ref(),
            distributor.mint.as_ref(),
            &id_bytes,
            &[distributor.bump],
        ];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.claimant_token_account.to_account_info(),
            authority: distributor.to_account_info(),
        };
        let signer = [signer_seeds];
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            &signer,
        );
        token::transfer(cpi_ctx, amount)?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(id: u64, _merkle_root: [u8; 32], max_claims: u32)]
pub struct InitializeDistributor<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = source_token_account.owner == authority.key() @ AirdropError::InvalidSourceOwner,
        constraint = source_token_account.mint == mint.key() @ AirdropError::InvalidMint,
    )]
    pub source_token_account: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = authority,
        space = 8 + Distributor::INIT_SPACE,
        seeds = [
            b"distributor",
            authority.key().as_ref(),
            mint.key().as_ref(),
            &id.to_le_bytes(),
        ],
        bump,
    )]
    pub distributor: Account<'info, Distributor>,

    #[account(
        init,
        payer = authority,
        token::mint = mint,
        token::authority = distributor,
        seeds = [b"vault", distributor.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = authority,
        space = 8 + ClaimBitmap::space(max_claims),
        seeds = [b"bitmap", distributor.key().as_ref()],
        bump,
    )]
    pub claim_bitmap: Account<'info, ClaimBitmap>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut)]
    pub claimant: Signer<'info>,

    #[account(
        mut,
        has_one = mint @ AirdropError::InvalidMint,
        has_one = vault @ AirdropError::InvalidVault,
    )]
    pub distributor: Account<'info, Distributor>,

    pub mint: Account<'info, Mint>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = claimant_token_account.owner == claimant.key() @ AirdropError::InvalidRecipientAccount,
        constraint = claimant_token_account.mint == mint.key() @ AirdropError::InvalidMint,
    )]
    pub claimant_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"bitmap", distributor.key().as_ref()],
        bump = claim_bitmap.bump,
        has_one = distributor @ AirdropError::InvalidBitmap,
    )]
    pub claim_bitmap: Account<'info, ClaimBitmap>,

    pub token_program: Program<'info, Token>,
}

#[account]
#[derive(InitSpace)]
pub struct Distributor {
    pub authority: Pubkey,
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

#[cfg(test)]
mod tests {
    use super::*;
    use primitivo::hash_pair;
    use std::str::FromStr;

    fn pk(s: &str) -> Pubkey {
        Pubkey::from_str(s).unwrap()
    }

    #[test]
    fn hash_pair_is_order_independent() {
        let a = hash_leaf(0, &pk("11111111111111111111111111111111"), 10);
        let b = hash_leaf(1, &pk("SysvarRent111111111111111111111111111111111"), 20);
        assert_eq!(hash_pair(&a, &b), hash_pair(&b, &a));
    }

    #[test]
    fn verify_proof_accepts_valid_two_leaf_proof() {
        let leaf_a = hash_leaf(0, &pk("11111111111111111111111111111111"), 10);
        let leaf_b = hash_leaf(1, &pk("SysvarRent111111111111111111111111111111111"), 20);
        let root = hash_pair(&leaf_a, &leaf_b);

        assert!(verify_proof(leaf_a, &[leaf_b], root));
        assert!(verify_proof(leaf_b, &[leaf_a], root));
    }

    #[test]
    fn verify_proof_rejects_invalid_proof() {
        let leaf_a = hash_leaf(0, &pk("11111111111111111111111111111111"), 10);
        let leaf_b = hash_leaf(1, &pk("SysvarRent111111111111111111111111111111111"), 20);
        let leaf_c = hash_leaf(2, &pk("SysvarC1ock11111111111111111111111111111111"), 30);
        let root = hash_pair(&leaf_a, &leaf_b);

        assert!(!verify_proof(leaf_a, &[leaf_c], root));
    }
}
