use solana_sdk::pubkey::Pubkey;

pub const SAVINGS_MANAGER_SEED_PREFIX: &[u8] = savings_program::SAVINGS_MANAGER_SEED_PREFIX;
pub const INTEREST_DISTRIBUTOR_SEED_PREFIX: &[u8] =
    savings_program::INTEREST_DISTRIBUTOR_SEED_PREFIX;

pub fn derive_savings_manager_pda(user: &Pubkey, distributor: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            SAVINGS_MANAGER_SEED_PREFIX,
            user.as_ref(),
            distributor.as_ref(),
        ],
        &savings_program::ID,
    )
}

pub fn derive_interest_distributor_pda(state: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            INTEREST_DISTRIBUTOR_SEED_PREFIX,
            state.as_ref(),
            mint.as_ref(),
        ],
        &savings_program::ID,
    )
}

pub fn derive_savings_vault_ata(mint: &Pubkey, savings_manager: &Pubkey) -> Pubkey {
    anchor_spl::associated_token::get_associated_token_address(savings_manager, mint)
}

pub fn derive_interest_vault_ata(mint: &Pubkey, distributor: &Pubkey) -> Pubkey {
    anchor_spl::associated_token::get_associated_token_address(distributor, mint)
}
