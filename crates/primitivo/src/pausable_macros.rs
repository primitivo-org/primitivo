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
                constraint = $state_account.ownership.owner == owner.key() @ $crate::PausableError::NotOwner,
            )]
            pub $state_account: Account<'info, $state_ty>,
        }

        #[derive(Accounts)]
        pub struct $unpause_ctx<'info> {
            #[account(mut)]
            pub owner: Signer<'info>,

            #[account(
                mut,
                constraint = $state_account.ownership.owner == owner.key() @ $crate::PausableError::NotOwner,
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
        pub fn $pause_fn(ctx: anchor_lang::prelude::Context<$pause_ctx>) -> anchor_lang::prelude::Result<()> {
            let owner = ctx.accounts.owner.key();
            let owner_of_state = ctx.accounts.$state_account.ownership.owner;
            let state = &mut ctx.accounts.$state_account;
            state.pausable.pause(owner_of_state, owner)?;
            Ok(())
        }

        pub fn $unpause_fn(ctx: anchor_lang::prelude::Context<$unpause_ctx>) -> anchor_lang::prelude::Result<()> {
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
