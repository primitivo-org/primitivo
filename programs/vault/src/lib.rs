#![allow(unexpected_cfgs)]
#![allow(deprecated)]

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, MintTo, Token, TokenAccount, Transfer};
use primitivo::{
    quote_deposit_shares, quote_redeem_underlying, Ownership, VaultError,
};

include!(concat!(env!("OUT_DIR"), "/vault_program_id.rs"));

#[program]
pub mod vault {
    use super::*;

    pub fn initialize_vault(ctx: Context<InitializeVault>, id: u64, derivative_decimals: u8) -> Result<()> {
        let cfg = &mut ctx.accounts.config;
        cfg.ownership = Ownership::new(ctx.accounts.authority.key());
        cfg.seed_authority = ctx.accounts.authority.key();
        cfg.underlying_mint = ctx.accounts.underlying_mint.key();
        cfg.derivative_mint = ctx.accounts.derivative_mint.key();
        cfg.underlying_vault = ctx.accounts.underlying_vault.key();
        cfg.id = id;
        cfg.underlying_assets = 0;
        cfg.derivative_decimals = derivative_decimals;
        cfg.bump = ctx.bumps.config;
        cfg.underlying_vault_bump = ctx.bumps.underlying_vault;
        cfg.derivative_mint_bump = ctx.bumps.derivative_mint;
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, underlying_amount: u64) -> Result<()> {
        let cfg = &mut ctx.accounts.config;
        let underlying_assets_before = ctx.accounts.underlying_vault.amount;
        let derivative_supply = ctx.accounts.derivative_mint.supply;

        let derivative_out =
            quote_deposit_shares(underlying_amount, underlying_assets_before, derivative_supply)?;

        let debit_accounts = Transfer {
            from: ctx.accounts.user_underlying_account.to_account_info(),
            to: ctx.accounts.underlying_vault.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let debit_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), debit_accounts);
        token::transfer(debit_ctx, underlying_amount)?;

        let id_bytes = cfg.id.to_le_bytes();
        let signer_seeds: &[&[u8]] = &[
            b"vault-config",
            cfg.seed_authority.as_ref(),
            cfg.underlying_mint.as_ref(),
            &id_bytes,
            &[cfg.bump],
        ];

        let mint_accounts = MintTo {
            mint: ctx.accounts.derivative_mint.to_account_info(),
            to: ctx.accounts.user_derivative_account.to_account_info(),
            authority: cfg.to_account_info(),
        };
        let signer = [signer_seeds];
        let mint_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            mint_accounts,
            &signer,
        );
        token::mint_to(mint_ctx, derivative_out)?;

        cfg.underlying_assets = underlying_assets_before
            .checked_add(underlying_amount)
            .ok_or(VaultError::ArithmeticOverflow)?;
        Ok(())
    }

    pub fn redeem(ctx: Context<Redeem>, derivative_amount: u64) -> Result<()> {
        let cfg = &mut ctx.accounts.config;
        let underlying_assets_before = ctx.accounts.underlying_vault.amount;
        let derivative_supply = ctx.accounts.derivative_mint.supply;

        let underlying_out =
            quote_redeem_underlying(derivative_amount, underlying_assets_before, derivative_supply)?;

        let burn_accounts = Burn {
            mint: ctx.accounts.derivative_mint.to_account_info(),
            from: ctx.accounts.user_derivative_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let burn_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), burn_accounts);
        token::burn(burn_ctx, derivative_amount)?;

        let id_bytes = cfg.id.to_le_bytes();
        let signer_seeds: &[&[u8]] = &[
            b"vault-config",
            cfg.seed_authority.as_ref(),
            cfg.underlying_mint.as_ref(),
            &id_bytes,
            &[cfg.bump],
        ];

        let transfer_accounts = Transfer {
            from: ctx.accounts.underlying_vault.to_account_info(),
            to: ctx.accounts.user_underlying_account.to_account_info(),
            authority: cfg.to_account_info(),
        };
        let signer = [signer_seeds];
        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            transfer_accounts,
            &signer,
        );
        token::transfer(transfer_ctx, underlying_out)?;

        cfg.underlying_assets = underlying_assets_before
            .checked_sub(underlying_out)
            .ok_or(VaultError::ArithmeticOverflow)?;
        Ok(())
    }

    pub fn propose_ownership_transfer(
        ctx: Context<ProposeVaultOwnershipTransfer>,
        new_owner: Pubkey,
        accept_window_secs: i64,
    ) -> Result<()> {
        propose_vault_ownership_transfer_impl(ctx, new_owner, accept_window_secs)
    }

    pub fn accept_ownership_transfer(ctx: Context<AcceptVaultOwnershipTransfer>) -> Result<()> {
        accept_vault_ownership_transfer_impl(ctx)
    }

    pub fn cancel_ownership_transfer(ctx: Context<CancelVaultOwnershipTransfer>) -> Result<()> {
        cancel_vault_ownership_transfer_impl(ctx)
    }
}

#[account]
#[derive(InitSpace)]
pub struct VaultConfig {
    pub ownership: Ownership,
    pub seed_authority: Pubkey,
    pub underlying_mint: Pubkey,
    pub derivative_mint: Pubkey,
    pub underlying_vault: Pubkey,
    pub id: u64,
    pub underlying_assets: u64,
    pub derivative_decimals: u8,
    pub bump: u8,
    pub underlying_vault_bump: u8,
    pub derivative_mint_bump: u8,
}

#[derive(Accounts)]
#[instruction(id: u64, derivative_decimals: u8)]
pub struct InitializeVault<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    pub underlying_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        space = 8 + VaultConfig::INIT_SPACE,
        seeds = [
            b"vault-config",
            authority.key().as_ref(),
            underlying_mint.key().as_ref(),
            &id.to_le_bytes(),
        ],
        bump,
    )]
    pub config: Account<'info, VaultConfig>,

    #[account(
        init,
        payer = authority,
        token::mint = underlying_mint,
        token::authority = config,
        seeds = [b"vault-underlying", config.key().as_ref()],
        bump,
    )]
    pub underlying_vault: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = authority,
        mint::decimals = derivative_decimals,
        mint::authority = config,
        seeds = [b"vault-derivative-mint", config.key().as_ref()],
        bump,
    )]
    pub derivative_mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        has_one = underlying_mint @ VaultError::InvalidState,
        has_one = derivative_mint @ VaultError::InvalidState,
        has_one = underlying_vault @ VaultError::InvalidState,
    )]
    pub config: Account<'info, VaultConfig>,

    pub underlying_mint: Account<'info, Mint>,

    #[account(mut)]
    pub derivative_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = user_underlying_account.owner == user.key() @ VaultError::InvalidState,
        constraint = user_underlying_account.mint == underlying_mint.key() @ VaultError::InvalidState,
    )]
    pub user_underlying_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_derivative_account.owner == user.key() @ VaultError::InvalidState,
        constraint = user_derivative_account.mint == derivative_mint.key() @ VaultError::InvalidState,
    )]
    pub user_derivative_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub underlying_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Redeem<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        has_one = underlying_mint @ VaultError::InvalidState,
        has_one = derivative_mint @ VaultError::InvalidState,
        has_one = underlying_vault @ VaultError::InvalidState,
    )]
    pub config: Account<'info, VaultConfig>,

    pub underlying_mint: Account<'info, Mint>,

    #[account(mut)]
    pub derivative_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = user_underlying_account.owner == user.key() @ VaultError::InvalidState,
        constraint = user_underlying_account.mint == underlying_mint.key() @ VaultError::InvalidState,
    )]
    pub user_underlying_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_derivative_account.owner == user.key() @ VaultError::InvalidState,
        constraint = user_derivative_account.mint == derivative_mint.key() @ VaultError::InvalidState,
    )]
    pub user_derivative_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub underlying_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

primitivo::generate_ownership_transfer_accounts!(
    state_ty = VaultConfig,
    state_account = config,
    propose_ctx = ProposeVaultOwnershipTransfer,
    accept_ctx = AcceptVaultOwnershipTransfer,
    cancel_ctx = CancelVaultOwnershipTransfer
);

primitivo::generate_ownership_transfer_handlers!(
    propose_fn = propose_vault_ownership_transfer_impl,
    accept_fn = accept_vault_ownership_transfer_impl,
    cancel_fn = cancel_vault_ownership_transfer_impl,
    propose_ctx = ProposeVaultOwnershipTransfer,
    accept_ctx = AcceptVaultOwnershipTransfer,
    cancel_ctx = CancelVaultOwnershipTransfer,
    state_account = config
);
