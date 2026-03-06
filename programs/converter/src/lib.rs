#![allow(unexpected_cfgs)]
#![allow(deprecated)]

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use converter_crate::{
    assert_minimum_received, quote_amount_out, update_rate_handler, validate_rate, ConverterConfig,
    ConverterError,
};
use primitivo_macro::Ownership;

include!(concat!(env!("OUT_DIR"), "/converter_program_id.rs"));

#[program]
pub mod converter {
    use super::*;

    pub fn initialize_converter(
        ctx: Context<InitializeConverter>,
        id: u64,
        rate_numerator: u64,
        rate_denominator: u64,
    ) -> Result<()> {
        validate_rate(rate_numerator, rate_denominator)?;

        let cfg = &mut ctx.accounts.config;
        cfg.ownership = Ownership::new(ctx.accounts.authority.key());
        cfg.seed_authority = ctx.accounts.authority.key();
        cfg.from_mint = ctx.accounts.from_mint.key();
        cfg.to_mint = ctx.accounts.to_mint.key();
        cfg.from_vault = ctx.accounts.from_vault.key();
        cfg.to_vault = ctx.accounts.to_vault.key();
        cfg.id = id;
        cfg.rate_numerator = rate_numerator;
        cfg.rate_denominator = rate_denominator;
        cfg.bump = ctx.bumps.config;
        cfg.from_vault_bump = ctx.bumps.from_vault;
        cfg.to_vault_bump = ctx.bumps.to_vault;

        Ok(())
    }

    pub fn update_rate(
        ctx: Context<UpdateRate>,
        rate_numerator: u64,
        rate_denominator: u64,
    ) -> Result<()> {
        let cfg = &mut ctx.accounts.config;
        update_rate_handler(
            &cfg.ownership,
            ctx.accounts.owner.key(),
            rate_numerator,
            rate_denominator,
        )?;

        cfg.rate_numerator = rate_numerator;
        cfg.rate_denominator = rate_denominator;
        Ok(())
    }

    pub fn swap(ctx: Context<Swap>, amount_in: u64, minimum_received: u64) -> Result<()> {
        let cfg = &ctx.accounts.config;
        let amount_out = quote_amount_out(amount_in, cfg.rate_numerator, cfg.rate_denominator)?;
        assert_minimum_received(amount_out, minimum_received)?;

        let debit_accounts = Transfer {
            from: ctx.accounts.user_from_account.to_account_info(),
            to: ctx.accounts.from_vault.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let debit_ctx =
            CpiContext::new(ctx.accounts.token_program.to_account_info(), debit_accounts);
        token::transfer(debit_ctx, amount_in)?;

        let id_bytes = cfg.id.to_le_bytes();
        let signer_seeds: &[&[u8]] = &[
            b"converter-config",
            cfg.seed_authority.as_ref(),
            cfg.from_mint.as_ref(),
            cfg.to_mint.as_ref(),
            &id_bytes,
            &[cfg.bump],
        ];

        let credit_accounts = Transfer {
            from: ctx.accounts.to_vault.to_account_info(),
            to: ctx.accounts.user_to_account.to_account_info(),
            authority: cfg.to_account_info(),
        };
        let signer = [signer_seeds];
        let credit_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            credit_accounts,
            &signer,
        );
        token::transfer(credit_ctx, amount_out)?;

        Ok(())
    }

    pub fn propose_ownership_transfer(
        ctx: Context<ProposeConverterOwnershipTransfer>,
        new_owner: Pubkey,
        accept_window_secs: i64,
    ) -> Result<()> {
        propose_converter_ownership_transfer_impl(ctx, new_owner, accept_window_secs)
    }

    pub fn accept_ownership_transfer(ctx: Context<AcceptConverterOwnershipTransfer>) -> Result<()> {
        accept_converter_ownership_transfer_impl(ctx)
    }

    pub fn cancel_ownership_transfer(ctx: Context<CancelConverterOwnershipTransfer>) -> Result<()> {
        cancel_converter_ownership_transfer_impl(ctx)
    }
}

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct InitializeConverter<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    pub from_mint: Account<'info, Mint>,
    pub to_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        space = 8 + ConverterConfig::INIT_SPACE,
        seeds = [
            b"converter-config",
            authority.key().as_ref(),
            from_mint.key().as_ref(),
            to_mint.key().as_ref(),
            &id.to_le_bytes(),
        ],
        bump,
    )]
    pub config: Account<'info, ConverterConfig>,

    #[account(
        init,
        payer = authority,
        token::mint = from_mint,
        token::authority = config,
        seeds = [b"converter-from-vault", config.key().as_ref()],
        bump,
    )]
    pub from_vault: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = authority,
        token::mint = to_mint,
        token::authority = config,
        seeds = [b"converter-to-vault", config.key().as_ref()],
        bump,
    )]
    pub to_vault: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct UpdateRate<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = config.ownership.owner == owner.key() @ ConverterError::NotOwner,
    )]
    pub config: Account<'info, ConverterConfig>,
}

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub config: Account<'info, ConverterConfig>,

    pub from_mint: Account<'info, Mint>,
    pub to_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = user_from_account.owner == user.key() @ ConverterError::NotOwner,
        constraint = user_from_account.mint == from_mint.key() @ ConverterError::InvalidRate,
    )]
    pub user_from_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_to_account.owner == user.key() @ ConverterError::NotOwner,
        constraint = user_to_account.mint == to_mint.key() @ ConverterError::InvalidRate,
    )]
    pub user_to_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = config.from_mint == from_mint.key() @ ConverterError::InvalidRate,
        constraint = config.from_vault == from_vault.key() @ ConverterError::InvalidRate,
    )]
    pub from_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = config.to_mint == to_mint.key() @ ConverterError::InvalidRate,
        constraint = config.to_vault == to_vault.key() @ ConverterError::InvalidRate,
    )]
    pub to_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

primitivo_macro::generate_ownership_transfer_accounts!(
    state_ty = ConverterConfig,
    state_account = config,
    propose_ctx = ProposeConverterOwnershipTransfer,
    accept_ctx = AcceptConverterOwnershipTransfer,
    cancel_ctx = CancelConverterOwnershipTransfer
);

primitivo_macro::generate_ownership_transfer_handlers!(
    propose_fn = propose_converter_ownership_transfer_impl,
    accept_fn = accept_converter_ownership_transfer_impl,
    cancel_fn = cancel_converter_ownership_transfer_impl,
    propose_ctx = ProposeConverterOwnershipTransfer,
    accept_ctx = AcceptConverterOwnershipTransfer,
    cancel_ctx = CancelConverterOwnershipTransfer,
    state_account = config
);
