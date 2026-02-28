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
                constraint = $state_account.ownership.owner == owner.key() @ $crate::OwnershipError::NotOwner,
            )]
            pub $state_account: Account<'info, $state_ty>,
        }

        #[derive(Accounts)]
        pub struct $accept_ctx<'info> {
            #[account(mut)]
            pub pending_owner: Signer<'info>,

            #[account(
                mut,
                constraint = $state_account.ownership.pending_owner == pending_owner.key() @ $crate::OwnershipError::InvalidPendingOwner,
            )]
            pub $state_account: Account<'info, $state_ty>,
        }

        #[derive(Accounts)]
        pub struct $cancel_ctx<'info> {
            #[account(mut)]
            pub owner: Signer<'info>,

            #[account(
                mut,
                constraint = $state_account.ownership.owner == owner.key() @ $crate::OwnershipError::NotOwner,
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
            ctx: anchor_lang::prelude::Context<$propose_ctx>,
            new_owner: anchor_lang::prelude::Pubkey,
            accept_window_secs: i64,
        ) -> anchor_lang::prelude::Result<()> {
            let now_ts = anchor_lang::prelude::Clock::get()?.unix_timestamp;
            ctx.accounts.$state_account.ownership.propose_transfer(
                ctx.accounts.owner.key(),
                new_owner,
                now_ts,
                accept_window_secs,
            )?;
            Ok(())
        }

        pub fn $accept_fn(
            ctx: anchor_lang::prelude::Context<$accept_ctx>,
        ) -> anchor_lang::prelude::Result<()> {
            let now_ts = anchor_lang::prelude::Clock::get()?.unix_timestamp;
            ctx.accounts.$state_account.ownership.accept_transfer(
                ctx.accounts.pending_owner.key(),
                now_ts,
            )?;
            Ok(())
        }

        pub fn $cancel_fn(
            ctx: anchor_lang::prelude::Context<$cancel_ctx>,
        ) -> anchor_lang::prelude::Result<()> {
            ctx.accounts
                .$state_account
                .ownership
                .cancel_transfer(ctx.accounts.owner.key())?;
            Ok(())
        }
    };
}
