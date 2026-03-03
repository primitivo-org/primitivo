use anchor_lang::prelude::*;

#[error_code]
pub enum PausableError {
    #[msg("Only owner can perform this action")]
    NotOwner,
    #[msg("Program is paused")]
    ProgramPaused,
    #[msg("Program is already paused")]
    AlreadyPaused,
    #[msg("Program is already unpaused")]
    AlreadyUnpaused,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct Pausable {
    pub paused: bool,
}

impl Pausable {
    pub fn new() -> Self {
        Self { paused: false }
    }

    pub fn pause(&mut self, owner: Pubkey, signer: Pubkey) -> Result<()> {
        require!(owner == signer, PausableError::NotOwner);
        require!(!self.paused, PausableError::AlreadyPaused);
        self.paused = true;
        Ok(())
    }

    pub fn unpause(&mut self, owner: Pubkey, signer: Pubkey) -> Result<()> {
        require!(owner == signer, PausableError::NotOwner);
        require!(self.paused, PausableError::AlreadyUnpaused);
        self.paused = false;
        Ok(())
    }

    pub fn require_not_paused(&self) -> Result<()> {
        require!(!self.paused, PausableError::ProgramPaused);
        Ok(())
    }
}
