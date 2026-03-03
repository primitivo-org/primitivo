// Stops Rust Analyzer complaining about missing configs
// See https://solana.stackexchange.com/questions/17777
#![allow(unexpected_cfgs)]
// Fix warning: use of deprecated method `anchor_lang::prelude::AccountInfo::<'a>::realloc`: Use AccountInfo::resize() instead
// See https://solana.stackexchange.com/questions/22979
#![allow(deprecated)]

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use airdrop_merkle::{
    claim_handler, initialize_distributor_handler, AirdropError, ClaimBitmap, Distributor,
};

include!(concat!(env!("OUT_DIR"), "/merke_airdrop_program_id.rs"));

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
        initialize_distributor_handler(
            ctx.accounts.distributor.key(),
            &mut ctx.accounts.distributor,
            &mut ctx.accounts.claim_bitmap,
            ctx.accounts.authority.key(),
            ctx.accounts.mint.key(),
            ctx.accounts.vault.key(),
            id,
            merkle_root,
            max_claims,
            ctx.bumps.distributor,
            ctx.bumps.vault,
            ctx.bumps.claim_bitmap,
        )?;

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
        let claimant = ctx.accounts.claimant.key();
        claim_handler(
            &mut ctx.accounts.distributor,
            &mut ctx.accounts.claim_bitmap,
            claimant,
            index,
            amount,
            &proof,
        )?;

        let distributor = &ctx.accounts.distributor;
        let id_bytes = distributor.id.to_le_bytes();
        let signer_seeds: &[&[u8]] = &[
            b"distributor",
            distributor.seed_authority.as_ref(),
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

    pub fn propose_ownership_transfer(
        ctx: Context<ProposeOwnershipTransfer>,
        new_owner: Pubkey,
        accept_window_secs: i64,
    ) -> Result<()> {
        propose_distributor_ownership_transfer_impl(ctx, new_owner, accept_window_secs)
    }

    pub fn accept_ownership_transfer(ctx: Context<AcceptOwnershipTransfer>) -> Result<()> {
        accept_distributor_ownership_transfer_impl(ctx)
    }

    pub fn cancel_ownership_transfer(ctx: Context<CancelOwnershipTransfer>) -> Result<()> {
        cancel_distributor_ownership_transfer_impl(ctx)
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

primitivo_macro::generate_ownership_transfer_accounts!(
    state_ty = Distributor,
    state_account = distributor,
    propose_ctx = ProposeOwnershipTransfer,
    accept_ctx = AcceptOwnershipTransfer,
    cancel_ctx = CancelOwnershipTransfer
);

primitivo_macro::generate_ownership_transfer_handlers!(
    propose_fn = propose_distributor_ownership_transfer_impl,
    accept_fn = accept_distributor_ownership_transfer_impl,
    cancel_fn = cancel_distributor_ownership_transfer_impl,
    propose_ctx = ProposeOwnershipTransfer,
    accept_ctx = AcceptOwnershipTransfer,
    cancel_ctx = CancelOwnershipTransfer,
    state_account = distributor
);
