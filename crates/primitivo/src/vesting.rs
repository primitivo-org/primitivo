use anchor_lang::prelude::*;

#[error_code]
pub enum VestingError {
    #[msg("Vesting total amount must be greater than 0")]
    InvalidTotalAmount,
    #[msg("Invalid vesting schedule timestamps")]
    InvalidSchedule,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
    #[msg("No claimable amount")]
    NoClaimableAmount,
    #[msg("Schedule already revoked")]
    AlreadyRevoked,
}

pub fn validate_vesting_params(total_amount: u64, start_ts: i64, cliff_ts: i64, end_ts: i64) -> Result<()> {
    require!(total_amount > 0, VestingError::InvalidTotalAmount);
    require!(start_ts <= cliff_ts, VestingError::InvalidSchedule);
    require!(cliff_ts <= end_ts, VestingError::InvalidSchedule);
    require!(end_ts > start_ts, VestingError::InvalidSchedule);
    Ok(())
}

pub fn effective_vesting_time(end_ts: i64, revoked_at: i64, now_ts: i64) -> i64 {
    let mut t = if now_ts < end_ts { now_ts } else { end_ts };
    if revoked_at > 0 && revoked_at < t {
        t = revoked_at;
    }
    t
}

pub fn vested_amount(total_amount: u64, start_ts: i64, cliff_ts: i64, end_ts: i64, at_ts: i64) -> Result<u64> {
    validate_vesting_params(total_amount, start_ts, cliff_ts, end_ts)?;

    if at_ts < cliff_ts {
        return Ok(0);
    }
    if at_ts >= end_ts {
        return Ok(total_amount);
    }

    let elapsed = (at_ts - start_ts).max(0) as u128;
    let duration = (end_ts - start_ts) as u128;
    let total = total_amount as u128;

    let vested = total
        .checked_mul(elapsed)
        .ok_or(VestingError::ArithmeticOverflow)?
        .checked_div(duration)
        .ok_or(VestingError::ArithmeticOverflow)?;

    u64::try_from(vested).map_err(|_| error!(VestingError::ArithmeticOverflow))
}

pub fn claimable_amount(
    total_amount: u64,
    released_amount: u64,
    start_ts: i64,
    cliff_ts: i64,
    end_ts: i64,
    revoked_at: i64,
    now_ts: i64,
) -> Result<u64> {
    let at_ts = effective_vesting_time(end_ts, revoked_at, now_ts);
    let vested = vested_amount(total_amount, start_ts, cliff_ts, end_ts, at_ts)?;
    vested
        .checked_sub(released_amount)
        .ok_or_else(|| error!(VestingError::ArithmeticOverflow))
}

pub fn increase_released_amount(released_amount: &mut u64, claim_amount: u64, total_amount: u64) -> Result<()> {
    require!(claim_amount > 0, VestingError::NoClaimableAmount);

    let next = released_amount
        .checked_add(claim_amount)
        .ok_or(VestingError::ArithmeticOverflow)?;
    require!(next <= total_amount, VestingError::ArithmeticOverflow);

    *released_amount = next;
    Ok(())
}

pub fn unvested_amount_on_revoke(total_amount: u64, start_ts: i64, cliff_ts: i64, end_ts: i64, now_ts: i64) -> Result<u64> {
    let at_ts = if now_ts < end_ts { now_ts } else { end_ts };
    let vested = vested_amount(total_amount, start_ts, cliff_ts, end_ts, at_ts)?;
    total_amount
        .checked_sub(vested)
        .ok_or_else(|| error!(VestingError::ArithmeticOverflow))
}
