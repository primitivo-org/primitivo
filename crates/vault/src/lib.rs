use anchor_lang::prelude::*;
use primitivo_macro::Ownership;

include!(concat!(env!("OUT_DIR"), "/primitivo_vault_program_id.rs"));

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

#[error_code]
pub enum VaultError {
    #[msg("Amount must be greater than 0")]
    InvalidAmount,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("Invalid state")]
    InvalidState,
    #[msg("Computed amount is zero")]
    ZeroAmountOut,
}

pub fn quote_deposit_shares(
    amount_in: u64,
    underlying_assets: u64,
    derivative_supply: u64,
) -> Result<u64> {
    require!(amount_in > 0, VaultError::InvalidAmount);

    if derivative_supply == 0 || underlying_assets == 0 {
        return Ok(amount_in);
    }

    let shares = (amount_in as u128)
        .checked_mul(derivative_supply as u128)
        .ok_or(VaultError::ArithmeticOverflow)?
        .checked_div(underlying_assets as u128)
        .ok_or(VaultError::ArithmeticOverflow)?;

    let shares = u64::try_from(shares).map_err(|_| error!(VaultError::ArithmeticOverflow))?;
    require!(shares > 0, VaultError::ZeroAmountOut);
    Ok(shares)
}

pub fn quote_redeem_underlying(
    derivative_in: u64,
    underlying_assets: u64,
    derivative_supply: u64,
) -> Result<u64> {
    require!(derivative_in > 0, VaultError::InvalidAmount);
    require!(derivative_supply > 0, VaultError::InvalidState);
    require!(underlying_assets > 0, VaultError::InvalidState);

    let underlying_out = (derivative_in as u128)
        .checked_mul(underlying_assets as u128)
        .ok_or(VaultError::ArithmeticOverflow)?
        .checked_div(derivative_supply as u128)
        .ok_or(VaultError::ArithmeticOverflow)?;

    let underlying_out =
        u64::try_from(underlying_out).map_err(|_| error!(VaultError::ArithmeticOverflow))?;
    require!(underlying_out > 0, VaultError::ZeroAmountOut);
    Ok(underlying_out)
}

pub fn increase_underlying_assets(underlying_assets: &mut u64, amount: u64) -> Result<()> {
    *underlying_assets = underlying_assets
        .checked_add(amount)
        .ok_or(VaultError::ArithmeticOverflow)?;
    Ok(())
}

pub fn decrease_underlying_assets(underlying_assets: &mut u64, amount: u64) -> Result<()> {
    *underlying_assets = underlying_assets
        .checked_sub(amount)
        .ok_or(VaultError::ArithmeticOverflow)?;
    Ok(())
}
