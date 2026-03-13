use trident_fuzz::fuzzing::*;

/// Storage for all account addresses used in fuzz testing.
///
/// This struct serves as a centralized repository for account addresses,
/// enabling their reuse across different instruction flows and test scenarios.
///
/// Docs: https://ackee.xyz/trident/docs/latest/trident-api-macro/trident-types/fuzz-accounts/
#[derive(Default)]
pub struct AccountAddresses {
    pub pending_owner: AddressStorage,

    pub config: AddressStorage,

    pub owner: AddressStorage,

    pub user: AddressStorage,

    pub underlying_mint: AddressStorage,

    pub derivative_mint: AddressStorage,

    pub user_underlying_account: AddressStorage,

    pub user_derivative_account: AddressStorage,

    pub underlying_vault: AddressStorage,

    pub token_program: AddressStorage,

    pub authority: AddressStorage,

    pub system_program: AddressStorage,

    pub distributor: AddressStorage,

    pub claimant: AddressStorage,

    pub mint: AddressStorage,

    pub vault: AddressStorage,

    pub claimant_token_account: AddressStorage,

    pub claim_bitmap: AddressStorage,

    pub source_token_account: AddressStorage,

    pub beneficiary: AddressStorage,

    pub schedule: AddressStorage,

    pub beneficiary_token_account: AddressStorage,

    pub revoke_destination: AddressStorage,

    pub from_mint: AddressStorage,

    pub to_mint: AddressStorage,

    pub from_vault: AddressStorage,

    pub to_vault: AddressStorage,

    pub user_from_account: AddressStorage,

    pub user_to_account: AddressStorage,
}
