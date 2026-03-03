pub mod ownership;
pub mod pausable;

pub use ownership::*;
pub use pausable::*;

#[macro_export]
macro_rules! generate_ownership_transfer_accounts {
    (
        state_ty = $state_ty:ident,
        state_account = $state_account:ident,
        propose_ctx = $propose_ctx:ident,
        accept_ctx = $accept_ctx:ident,
        cancel_ctx = $cancel_ctx:ident
    ) => {
        #[derive(Accounts)]
        pub struct $propose_ctx<'info> {
            #[account(mut)]
            pub owner: Signer<'info>,

            #[account(
                mut,
                constraint = $state_account.ownership.owner == owner.key() @ primitivo_macro::OwnershipError::NotOwner,
            )]
            pub $state_account: Account<'info, $state_ty>,
        }

        #[derive(Accounts)]
        pub struct $accept_ctx<'info> {
            #[account(mut)]
            pub pending_owner: Signer<'info>,

            #[account(
                mut,
                constraint = $state_account.ownership.pending_owner == pending_owner.key() @ primitivo_macro::OwnershipError::InvalidPendingOwner,
            )]
            pub $state_account: Account<'info, $state_ty>,
        }

        #[derive(Accounts)]
        pub struct $cancel_ctx<'info> {
            #[account(mut)]
            pub owner: Signer<'info>,

            #[account(
                mut,
                constraint = $state_account.ownership.owner == owner.key() @ primitivo_macro::OwnershipError::NotOwner,
            )]
            pub $state_account: Account<'info, $state_ty>,
        }
    };
}

#[macro_export]
macro_rules! generate_ownership_transfer_handlers {
    (
        propose_fn = $propose_fn:ident,
        accept_fn = $accept_fn:ident,
        cancel_fn = $cancel_fn:ident,
        propose_ctx = $propose_ctx:ident,
        accept_ctx = $accept_ctx:ident,
        cancel_ctx = $cancel_ctx:ident,
        state_account = $state_account:ident
    ) => {
        pub fn $propose_fn(
            ctx: Context<$propose_ctx>,
            new_owner: Pubkey,
            accept_window_secs: i64,
        ) -> Result<()> {
            let now_ts = Clock::get()?.unix_timestamp;
            ctx.accounts.$state_account.ownership.propose_transfer(
                ctx.accounts.owner.key(),
                new_owner,
                now_ts,
                accept_window_secs,
            )?;
            Ok(())
        }

        pub fn $accept_fn(
            ctx: Context<$accept_ctx>,
        ) -> Result<()> {
            let now_ts = Clock::get()?.unix_timestamp;
            ctx.accounts.$state_account.ownership.accept_transfer(
                ctx.accounts.pending_owner.key(),
                now_ts,
            )?;
            Ok(())
        }

        pub fn $cancel_fn(
            ctx: Context<$cancel_ctx>,
        ) -> Result<()> {
            ctx.accounts
                .$state_account
                .ownership
                .cancel_transfer(ctx.accounts.owner.key())?;
            Ok(())
        }
    };
}

#[macro_export]
macro_rules! generate_pausable_accounts {
    (
        state_ty = $state_ty:ident,
        state_account = $state_account:ident,
        pause_ctx = $pause_ctx:ident,
        unpause_ctx = $unpause_ctx:ident
    ) => {
        #[derive(Accounts)]
        pub struct $pause_ctx<'info> {
            #[account(mut)]
            pub owner: Signer<'info>,

            #[account(
                mut,
                constraint = $state_account.ownership.owner == owner.key() @ primitivo_macro::PausableError::NotOwner,
            )]
            pub $state_account: Account<'info, $state_ty>,
        }

        #[derive(Accounts)]
        pub struct $unpause_ctx<'info> {
            #[account(mut)]
            pub owner: Signer<'info>,

            #[account(
                mut,
                constraint = $state_account.ownership.owner == owner.key() @ primitivo_macro::PausableError::NotOwner,
            )]
            pub $state_account: Account<'info, $state_ty>,
        }
    };
}

#[macro_export]
macro_rules! generate_pausable_handlers {
    (
        pause_fn = $pause_fn:ident,
        unpause_fn = $unpause_fn:ident,
        pause_ctx = $pause_ctx:ident,
        unpause_ctx = $unpause_ctx:ident,
        state_account = $state_account:ident
    ) => {
        pub fn $pause_fn(
            ctx: Context<$pause_ctx>,
        ) -> Result<()> {
            let owner = ctx.accounts.owner.key();
            let owner_of_state = ctx.accounts.$state_account.ownership.owner;
            let state = &mut ctx.accounts.$state_account;
            state.pausable.pause(owner_of_state, owner)?;
            Ok(())
        }

        pub fn $unpause_fn(
            ctx: Context<$unpause_ctx>,
        ) -> Result<()> {
            let owner = ctx.accounts.owner.key();
            let owner_of_state = ctx.accounts.$state_account.ownership.owner;
            let state = &mut ctx.accounts.$state_account;
            state.pausable.unpause(owner_of_state, owner)?;
            Ok(())
        }
    };
}

#[macro_export]
macro_rules! require_not_paused {
    ($ctx:expr, $state_account:ident) => {
        $ctx.accounts.$state_account.pausable.require_not_paused()?;
    };
}
