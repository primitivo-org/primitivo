use anchor_lang::{AccountDeserialize, AnchorSerialize};
use converter_crate::ConverterConfig;
use fuzz_accounts::*;
use primitivo_macro::Ownership;
use trident_fuzz::fuzzing::*;
mod fuzz_accounts;

#[derive(FuzzTestMethods)]
struct FuzzTest {
    /// Trident client for interacting with the Solana program
    trident: Trident,
    /// Storage for all account addresses used in fuzz testing
    fuzz_accounts: AccountAddresses,
}

#[flow_executor]
impl FuzzTest {
    fn new() -> Self {
        Self {
            trident: Trident::default(),
            fuzz_accounts: AccountAddresses::default(),
        }
    }

    #[init]
    fn start(&mut self) {
        let owner = self.trident.payer().pubkey();
        let config = self.fuzz_accounts.config.insert(&mut self.trident, None);
        let from_mint = self.fuzz_accounts.from_mint.insert(&mut self.trident, None);
        let to_mint = self.fuzz_accounts.to_mint.insert(&mut self.trident, None);
        let from_vault = self
            .fuzz_accounts
            .from_vault
            .insert(&mut self.trident, None);
        let to_vault = self.fuzz_accounts.to_vault.insert(&mut self.trident, None);

        self.fuzz_accounts.owner.insert_with_address(owner);

        seed_converter_config(
            &mut self.trident,
            config,
            owner,
            from_mint,
            to_mint,
            from_vault,
            to_vault,
        );
    }

    #[flow]
    fn flow1(&mut self) {
        let owner = self
            .fuzz_accounts
            .owner
            .get(&mut self.trident)
            .expect("owner must be seeded");
        let config = self
            .fuzz_accounts
            .config
            .get(&mut self.trident)
            .expect("config must be seeded");

        let new_owner = random_distinct_pubkey(&mut self.trident, owner);
        let accept_window_secs = self.trident.random_from_range(1_i64..=3000_i64);

        let instruction =
            propose_ownership_transfer_instruction(owner, config, new_owner, accept_window_secs);

        let result = self
            .trident
            .process_transaction(&[instruction], Some("converter_propose_ownership_transfer"));

        assert!(result.is_success(), "{}", result.logs());

        let config_state = load_converter_config(&mut self.trident, &config);
        assert_eq!(config_state.ownership.owner, owner);
        assert_eq!(config_state.ownership.pending_owner, new_owner);
        assert!(config_state.ownership.pending_expires_at > 0);
    }

    #[flow]
    fn flow2(&mut self) {
        let owner = self
            .fuzz_accounts
            .owner
            .get(&mut self.trident)
            .expect("owner must be seeded");
        let config = self
            .fuzz_accounts
            .config
            .get(&mut self.trident)
            .expect("config must be seeded");

        let pending_owner = random_distinct_pubkey(&mut self.trident, owner);
        let propose = propose_ownership_transfer_instruction(owner, config, pending_owner, 60);

        let propose_result = self
            .trident
            .process_transaction(&[propose], Some("converter_propose_before_cancel"));
        assert!(propose_result.is_success(), "{}", propose_result.logs());

        let cancel = cancel_ownership_transfer_instruction(owner, config);

        let cancel_result = self
            .trident
            .process_transaction(&[cancel], Some("converter_cancel_ownership_transfer"));
        assert!(cancel_result.is_success(), "{}", cancel_result.logs());

        let config_state = load_converter_config(&mut self.trident, &config);
        assert_eq!(config_state.ownership.owner, owner);
        assert_eq!(config_state.ownership.pending_owner, Pubkey::default());
        assert_eq!(config_state.ownership.pending_expires_at, 0);
    }

    #[end]
    fn end(&mut self) {
        // Perform any cleanup here, this method will be executed
        // at the end of each iteration
    }
}

fn main() {
    FuzzTest::fuzz(250, 80);
}

fn random_distinct_pubkey(trident: &mut Trident, not_equal: Pubkey) -> Pubkey {
    loop {
        let candidate = trident.random_pubkey();
        if candidate != not_equal && candidate != Pubkey::default() {
            return candidate;
        }
    }
}

fn seed_converter_config(
    trident: &mut Trident,
    config_address: Pubkey,
    owner: Pubkey,
    from_mint: Pubkey,
    to_mint: Pubkey,
    from_vault: Pubkey,
    to_vault: Pubkey,
) {
    let config = ConverterConfig {
        ownership: Ownership::new(owner),
        seed_authority: owner,
        from_mint,
        to_mint,
        from_vault,
        to_vault,
        id: 0,
        rate_numerator: 1,
        rate_denominator: 1,
        bump: 0,
        from_vault_bump: 0,
        to_vault_bump: 0,
    };

    let mut data = converter_config_discriminator().to_vec();
    data.extend(
        config
            .try_to_vec()
            .expect("converter config should serialize"),
    );

    let mut account = AccountSharedData::new(
        data.len() as u64 + LAMPORTS_PER_SOL,
        data.len(),
        &converter_program_id(),
    );
    account.set_data_from_slice(&data);
    trident.set_account_custom(&config_address, &account);
}

fn load_converter_config(trident: &mut Trident, config: &Pubkey) -> ConverterConfig {
    let account = trident.get_account(config);
    let mut data = account.data();
    ConverterConfig::try_deserialize(&mut data).expect("converter config should deserialize")
}

fn converter_program_id() -> Pubkey {
    pubkey!("87ReJzgQU1KDbRWRtGC1DTtxUZ6dE3MkcEx2a37fFySr")
}

fn converter_config_discriminator() -> [u8; 8] {
    [227, 71, 47, 133, 181, 199, 41, 254]
}

fn propose_ownership_transfer_instruction(
    owner: Pubkey,
    config: Pubkey,
    new_owner: Pubkey,
    accept_window_secs: i64,
) -> Instruction {
    let mut data = vec![5_u8, 78, 67, 196, 223, 159, 228, 136];
    data.extend_from_slice(new_owner.as_ref());
    data.extend_from_slice(&accept_window_secs.to_le_bytes());

    Instruction {
        program_id: converter_program_id(),
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(config, false),
        ],
        data,
    }
}

fn cancel_ownership_transfer_instruction(owner: Pubkey, config: Pubkey) -> Instruction {
    Instruction {
        program_id: converter_program_id(),
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(config, false),
        ],
        data: vec![2_u8, 184, 195, 105, 138, 142, 154, 75],
    }
}
