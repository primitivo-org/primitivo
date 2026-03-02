use anchor_lang::prelude::*;

use crate::Ownership;

#[error_code]
pub enum ConverterError {
    #[msg("Only owner can update rate")]
    NotOwner,
    #[msg("Invalid exchange rate")]
    InvalidRate,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("Output amount is 0")]
    ZeroOutput,
    #[msg("Minimum received not satisfied")]
    SlippageExceeded,
}

pub fn validate_rate(rate_numerator: u64, rate_denominator: u64) -> Result<()> {
    require!(rate_numerator > 0, ConverterError::InvalidRate);
    require!(rate_denominator > 0, ConverterError::InvalidRate);
    Ok(())
}

pub fn quote_amount_out(amount_in: u64, rate_numerator: u64, rate_denominator: u64) -> Result<u64> {
    validate_rate(rate_numerator, rate_denominator)?;

    let amount_out = (amount_in as u128)
        .checked_mul(rate_numerator as u128)
        .ok_or(ConverterError::ArithmeticOverflow)?
        .checked_div(rate_denominator as u128)
        .ok_or(ConverterError::ArithmeticOverflow)?;

    let amount_out = u64::try_from(amount_out).map_err(|_| error!(ConverterError::ArithmeticOverflow))?;
    require!(amount_out > 0, ConverterError::ZeroOutput);
    Ok(amount_out)
}

pub fn update_rate_handler(
    ownership: &Ownership,
    signer: Pubkey,
    rate_numerator: u64,
    rate_denominator: u64,
) -> Result<()> {
    ownership
        .require_owner(signer)
        .map_err(|_| error!(ConverterError::NotOwner))?;
    validate_rate(rate_numerator, rate_denominator)
}

pub fn assert_minimum_received(amount_out: u64, minimum_received: u64) -> Result<()> {
    require!(amount_out >= minimum_received, ConverterError::SlippageExceeded);
    Ok(())
}
