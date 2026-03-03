use anchor_lang::prelude::*;

#[error_code]
pub enum OwnershipError {
    #[msg("Only current owner can perform this action")]
    NotOwner,
    #[msg("No pending ownership transfer")]
    NoPendingOwnership,
    #[msg("Only pending owner can accept ownership")]
    InvalidPendingOwner,
    #[msg("Pending ownership transfer has expired")]
    PendingOwnershipExpired,
    #[msg("Ownership accept window must be greater than 0")]
    InvalidAcceptWindow,
    #[msg("New owner must be different and non-default")]
    InvalidNewOwner,
    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct Ownership {
    pub owner: Pubkey,
    pub pending_owner: Pubkey,
    pub pending_expires_at: i64,
}

impl Ownership {
    pub fn new(owner: Pubkey) -> Self {
        Self {
            owner,
            pending_owner: Pubkey::default(),
            pending_expires_at: 0,
        }
    }

    pub fn has_pending(&self) -> bool {
        self.pending_owner != Pubkey::default()
    }

    pub fn require_owner(&self, signer: Pubkey) -> Result<()> {
        require!(self.owner == signer, OwnershipError::NotOwner);
        Ok(())
    }

    pub fn propose_transfer(
        &mut self,
        signer: Pubkey,
        new_owner: Pubkey,
        now_ts: i64,
        accept_window_secs: i64,
    ) -> Result<()> {
        self.require_owner(signer)?;
        require!(accept_window_secs > 0, OwnershipError::InvalidAcceptWindow);
        require!(
            new_owner != Pubkey::default() && new_owner != self.owner,
            OwnershipError::InvalidNewOwner
        );

        let expires_at = now_ts
            .checked_add(accept_window_secs)
            .ok_or(OwnershipError::ArithmeticOverflow)?;

        self.pending_owner = new_owner;
        self.pending_expires_at = expires_at;
        Ok(())
    }

    pub fn accept_transfer(&mut self, signer: Pubkey, now_ts: i64) -> Result<()> {
        require!(self.has_pending(), OwnershipError::NoPendingOwnership);
        require!(self.pending_owner == signer, OwnershipError::InvalidPendingOwner);

        if now_ts > self.pending_expires_at {
            self.clear_pending();
            return err!(OwnershipError::PendingOwnershipExpired);
        }

        self.owner = signer;
        self.clear_pending();
        Ok(())
    }

    pub fn cancel_transfer(&mut self, signer: Pubkey) -> Result<()> {
        self.require_owner(signer)?;
        require!(self.has_pending(), OwnershipError::NoPendingOwnership);
        self.clear_pending();
        Ok(())
    }

    fn clear_pending(&mut self) {
        self.pending_owner = Pubkey::default();
        self.pending_expires_at = 0;
    }
}
