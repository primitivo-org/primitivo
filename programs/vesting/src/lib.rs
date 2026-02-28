#![allow(unexpected_cfgs)]
#![allow(deprecated)]

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use primitivo::{
    claimable_amount, increase_released_amount, unvested_amount_on_revoke, validate_vesting_params,
    Ownership, OwnershipError, VestingError,
};

include!(concat!(env!("OUT_DIR"), "/vesting_program_id.rs"));

#[program]
pub mod vesting {
    use super::*;

    pub fn initialize_vesting_config(ctx: Context<InitializeVestingConfig>, id: u64) -> Result<()> {
        let cfg = &mut ctx.accounts.config;
        cfg.ownership = Ownership::new(ctx.accounts.authority.key());
        cfg.seed_authority = ctx.accounts.authority.key();
        cfg.mint = ctx.accounts.mint.key();
        cfg.vault = ctx.accounts.vault.key();
        cfg.id = id;
        cfg.bump = ctx.bumps.config;
        cfg.vault_bump = ctx.bumps.vault;
        Ok(())
    }

    pub fn create_schedule(
        ctx: Context<CreateSchedule>,
        total_amount: u64,
        start_ts: i64,
        cliff_ts: i64,
        end_ts: i64,
    ) -> Result<()> {
        ctx.accounts
            .config
            .ownership
            .require_owner(ctx.accounts.authority.key())?;
        validate_vesting_params(total_amount, start_ts, cliff_ts, end_ts)?;

        let schedule = &mut ctx.accounts.schedule;
        schedule.config = ctx.accounts.config.key();
        schedule.beneficiary = ctx.accounts.beneficiary.key();
        schedule.total_amount = total_amount;
        schedule.released_amount = 0;
        schedule.start_ts = start_ts;
        schedule.cliff_ts = cliff_ts;
        schedule.end_ts = end_ts;
        schedule.revoked_at = 0;
        schedule.bump = ctx.bumps.schedule;

        let cpi_accounts = Transfer {
            from: ctx.accounts.source_token_account.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, total_amount)?;

        Ok(())
    }

    pub fn claim(ctx: Context<ClaimVested>) -> Result<()> {
        let now_ts = Clock::get()?.unix_timestamp;
        let schedule = &mut ctx.accounts.schedule;

        let claim_amount = claimable_amount(
            schedule.total_amount,
            schedule.released_amount,
            schedule.start_ts,
            schedule.cliff_ts,
            schedule.end_ts,
            schedule.revoked_at,
            now_ts,
        )?;
        let total_amount = schedule.total_amount;
        increase_released_amount(
            &mut schedule.released_amount,
            claim_amount,
            total_amount,
        )?;

        let cfg = &ctx.accounts.config;
        let id_bytes = cfg.id.to_le_bytes();
        let signer_seeds: &[&[u8]] = &[
            b"vesting-config",
            cfg.seed_authority.as_ref(),
            cfg.mint.as_ref(),
            &id_bytes,
            &[cfg.bump],
        ];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.beneficiary_token_account.to_account_info(),
            authority: cfg.to_account_info(),
        };
        let signer = [signer_seeds];
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            &signer,
        );
        token::transfer(cpi_ctx, claim_amount)?;

        Ok(())
    }

    pub fn revoke(ctx: Context<RevokeSchedule>) -> Result<()> {
        ctx.accounts
            .config
            .ownership
            .require_owner(ctx.accounts.authority.key())?;
        let now_ts = Clock::get()?.unix_timestamp;
        let schedule = &mut ctx.accounts.schedule;

        require!(schedule.revoked_at == 0, VestingError::AlreadyRevoked);

        let unvested_amount = unvested_amount_on_revoke(
            schedule.total_amount,
            schedule.start_ts,
            schedule.cliff_ts,
            schedule.end_ts,
            now_ts,
        )?;

        schedule.revoked_at = now_ts;

        if unvested_amount > 0 {
            let cfg = &ctx.accounts.config;
            let id_bytes = cfg.id.to_le_bytes();
            let signer_seeds: &[&[u8]] = &[
                b"vesting-config",
                cfg.seed_authority.as_ref(),
                cfg.mint.as_ref(),
                &id_bytes,
                &[cfg.bump],
            ];

            let cpi_accounts = Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.revoke_destination.to_account_info(),
                authority: cfg.to_account_info(),
            };
            let signer = [signer_seeds];
            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                cpi_accounts,
                &signer,
            );
            token::transfer(cpi_ctx, unvested_amount)?;
        }

        Ok(())
    }

    pub fn propose_ownership_transfer(
        ctx: Context<ProposeVestingOwnershipTransfer>,
        new_owner: Pubkey,
        accept_window_secs: i64,
    ) -> Result<()> {
        let now_ts = Clock::get()?.unix_timestamp;
        ctx.accounts.config.ownership.propose_transfer(
            ctx.accounts.owner.key(),
            new_owner,
            now_ts,
            accept_window_secs,
        )?;
        Ok(())
    }

    pub fn accept_ownership_transfer(ctx: Context<AcceptVestingOwnershipTransfer>) -> Result<()> {
        let now_ts = Clock::get()?.unix_timestamp;
        ctx.accounts
            .config
            .ownership
            .accept_transfer(ctx.accounts.pending_owner.key(), now_ts)?;
        Ok(())
    }

    pub fn cancel_ownership_transfer(ctx: Context<CancelVestingOwnershipTransfer>) -> Result<()> {
        ctx.accounts
            .config
            .ownership
            .cancel_transfer(ctx.accounts.owner.key())?;
        Ok(())
    }
}

#[account]
#[derive(InitSpace)]
pub struct VestingConfig {
    pub ownership: Ownership,
    pub seed_authority: Pubkey,
    pub mint: Pubkey,
    pub vault: Pubkey,
    pub id: u64,
    pub bump: u8,
    pub vault_bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct VestingSchedule {
    pub config: Pubkey,
    pub beneficiary: Pubkey,
    pub total_amount: u64,
    pub released_amount: u64,
    pub start_ts: i64,
    pub cliff_ts: i64,
    pub end_ts: i64,
    pub revoked_at: i64,
    pub bump: u8,
}

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct InitializeVestingConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        space = 8 + VestingConfig::INIT_SPACE,
        seeds = [b"vesting-config", authority.key().as_ref(), mint.key().as_ref(), &id.to_le_bytes()],
        bump,
    )]
    pub config: Account<'info, VestingConfig>,

    #[account(
        init,
        payer = authority,
        token::mint = mint,
        token::authority = config,
        seeds = [b"vesting-vault", config.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CreateSchedule<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    pub beneficiary: SystemAccount<'info>,

    #[account(
        mut,
        has_one = mint,
        has_one = vault,
        constraint = config.ownership.owner == authority.key() @ VestingError::InvalidSchedule,
    )]
    pub config: Account<'info, VestingConfig>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = source_token_account.owner == authority.key() @ VestingError::InvalidSchedule,
        constraint = source_token_account.mint == mint.key() @ VestingError::InvalidSchedule,
    )]
    pub source_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = authority,
        space = 8 + VestingSchedule::INIT_SPACE,
        seeds = [b"vesting-schedule", config.key().as_ref(), beneficiary.key().as_ref()],
        bump,
    )]
    pub schedule: Account<'info, VestingSchedule>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ClaimVested<'info> {
    #[account(mut)]
    pub beneficiary: Signer<'info>,

    #[account(mut)]
    pub config: Account<'info, VestingConfig>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        has_one = config,
        has_one = beneficiary,
    )]
    pub schedule: Account<'info, VestingSchedule>,

    #[account(
        mut,
        constraint = beneficiary_token_account.owner == beneficiary.key() @ VestingError::InvalidSchedule,
        constraint = beneficiary_token_account.mint == mint.key() @ VestingError::InvalidSchedule,
    )]
    pub beneficiary_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = config.mint == mint.key() @ VestingError::InvalidSchedule,
        constraint = config.vault == vault.key() @ VestingError::InvalidSchedule,
    )]
    pub vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RevokeSchedule<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = mint,
        has_one = vault,
        constraint = config.ownership.owner == authority.key() @ VestingError::InvalidSchedule,
    )]
    pub config: Account<'info, VestingConfig>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        has_one = config,
    )]
    pub schedule: Account<'info, VestingSchedule>,

    #[account(
        mut,
        constraint = revoke_destination.owner == authority.key() @ VestingError::InvalidSchedule,
        constraint = revoke_destination.mint == mint.key() @ VestingError::InvalidSchedule,
    )]
    pub revoke_destination: Account<'info, TokenAccount>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ProposeVestingOwnershipTransfer<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = config.ownership.owner == owner.key() @ OwnershipError::NotOwner,
    )]
    pub config: Account<'info, VestingConfig>,
}

#[derive(Accounts)]
pub struct AcceptVestingOwnershipTransfer<'info> {
    #[account(mut)]
    pub pending_owner: Signer<'info>,

    #[account(
        mut,
        constraint = config.ownership.pending_owner == pending_owner.key() @ OwnershipError::InvalidPendingOwner,
    )]
    pub config: Account<'info, VestingConfig>,
}

#[derive(Accounts)]
pub struct CancelVestingOwnershipTransfer<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = config.ownership.owner == owner.key() @ OwnershipError::NotOwner,
    )]
    pub config: Account<'info, VestingConfig>,
}
