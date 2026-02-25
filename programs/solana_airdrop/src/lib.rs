// Stops Rust Analyzer complaining about missing configs
// See https://solana.stackexchange.com/questions/17777
#![allow(unexpected_cfgs)]
// Fix warning: use of deprecated method `anchor_lang::prelude::AccountInfo::<'a>::realloc`: Use AccountInfo::resize() instead
// See https://solana.stackexchange.com/questions/22979
#![allow(deprecated)]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("5DmKC7Umm2ntYMHPnfjmUCsyqjE4rRbU2we9Vw6KzPPi");

#[program]
pub mod solana_airdrop {
    use super::*;

    pub fn initialize_distributor(
        ctx: Context<InitializeDistributor>,
        id: u64,
        merkle_root: [u8; 32],
        total_funding_amount: u64,
    ) -> Result<()> {
        let distributor = &mut ctx.accounts.distributor;
        distributor.authority = ctx.accounts.authority.key();
        distributor.mint = ctx.accounts.mint.key();
        distributor.vault = ctx.accounts.vault.key();
        distributor.merkle_root = merkle_root;
        distributor.id = id;
        distributor.claimed_amount = 0;
        distributor.bump = ctx.bumps.distributor;
        distributor.vault_bump = ctx.bumps.vault;

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

    pub fn claim(ctx: Context<Claim>, amount: u64, proof: Vec<[u8; 32]>) -> Result<()> {
        require!(amount > 0, AirdropError::InvalidClaimAmount);

        let distributor = &mut ctx.accounts.distributor;
        let claimant = ctx.accounts.claimant.key();

        let leaf = hash_leaf(&claimant, amount);
        require!(
            verify_proof(leaf, &proof, distributor.merkle_root),
            AirdropError::InvalidProof
        );

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

        let receipt = &mut ctx.accounts.claim_receipt;
        receipt.distributor = distributor.key();
        receipt.claimant = claimant;
        receipt.bump = ctx.bumps.claim_receipt;
        receipt.claimed_slot = Clock::get()?.slot;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(id: u64)]
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
        space = 8 + Distributor::LEN,
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
        init,
        payer = claimant,
        space = 8 + ClaimReceipt::LEN,
        seeds = [b"claim", distributor.key().as_ref(), claimant.key().as_ref()],
        bump,
    )]
    pub claim_receipt: Account<'info, ClaimReceipt>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct Distributor {
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub merkle_root: [u8; 32],
    pub id: u64,
    pub claimed_amount: u64,
    pub bump: u8,
    pub vault_bump: u8,
}

impl Distributor {
    pub const LEN: usize = (32 * 4) + (8 * 2) + 2;
}

#[account]
pub struct ClaimReceipt {
    pub distributor: Pubkey,
    pub claimant: Pubkey,
    pub bump: u8,
    pub claimed_slot: u64,
}

impl ClaimReceipt {
    pub const LEN: usize = 32 + 32 + 1 + 8;
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
}

pub fn hash_leaf(recipient: &Pubkey, amount: u64) -> [u8; 32] {
    let amount_bytes = amount.to_le_bytes();
    hashv(&[b"merkle_airdrop", recipient.as_ref(), &amount_bytes]).to_bytes()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn pk(s: &str) -> Pubkey {
        Pubkey::from_str(s).unwrap()
    }

    #[test]
    fn hash_pair_is_order_independent() {
        let a = hash_leaf(&pk("11111111111111111111111111111111"), 10);
        let b = hash_leaf(&pk("SysvarRent111111111111111111111111111111111"), 20);
        assert_eq!(hash_pair(&a, &b), hash_pair(&b, &a));
    }

    #[test]
    fn verify_proof_accepts_valid_two_leaf_proof() {
        let leaf_a = hash_leaf(&pk("11111111111111111111111111111111"), 10);
        let leaf_b = hash_leaf(&pk("SysvarRent111111111111111111111111111111111"), 20);
        let root = hash_pair(&leaf_a, &leaf_b);

        assert!(verify_proof(leaf_a, &[leaf_b], root));
        assert!(verify_proof(leaf_b, &[leaf_a], root));
    }

    #[test]
    fn verify_proof_rejects_invalid_proof() {
        let leaf_a = hash_leaf(&pk("11111111111111111111111111111111"), 10);
        let leaf_b = hash_leaf(&pk("SysvarRent111111111111111111111111111111111"), 20);
        let leaf_c = hash_leaf(&pk("SysvarC1ock11111111111111111111111111111111"), 30);
        let root = hash_pair(&leaf_a, &leaf_b);

        assert!(!verify_proof(leaf_a, &[leaf_c], root));
    }
}
