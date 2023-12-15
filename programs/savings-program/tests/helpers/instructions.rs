use anchor_lang::{InstructionData, ToAccountMetas};
use savings_program::accounts::*;
use savings_program::instruction;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, system_program};

pub fn initialize_state(
    initializer: &Pubkey,
    state: &Pubkey,
    authority: Option<&Pubkey>,
) -> (InitializeState, Instruction) {
    let accounts = InitializeState {
        initializer: *initializer,
        state: *state,
        system_program: system_program::ID,
    };

    let data = instruction::InitializeState {
        authority: *authority.unwrap_or(initializer),
    }
    .data();

    let instruction = Instruction {
        program_id: savings_program::ID,
        data,
        accounts: accounts.to_account_metas(None),
    };

    (accounts, instruction)
}

pub fn create_interest_vault(
    payer: &Pubkey,
    authority: &Pubkey,
    state: &Pubkey,
    mint: &Pubkey,
    distributor: &Pubkey,
    interest_vault: &Pubkey,
) -> (CreateInterestVaultForMint, Instruction) {
    let accounts = CreateInterestVaultForMint {
        authority: *authority,
        payer: *payer,
        state: *state,
        mint: *mint,
        interest_distributor: *distributor,
        interest_vault: *interest_vault,
        system_program: system_program::id(),
        token_program: anchor_spl::token::ID,
        associated_token_program: anchor_spl::associated_token::ID,
    };

    let data = instruction::CreateInterestVault {}.data();

    let instruction = Instruction {
        program_id: savings_program::ID,
        data,
        accounts: accounts.to_account_metas(None),
    };

    (accounts, instruction)
}
pub fn deposit_to_interest_vault(
    authority: &Pubkey,
    state: &Pubkey,
    depositor: &Pubkey,
    depositor_token_account: &Pubkey,
    interest_distributor: &Pubkey,
    interest_vault: &Pubkey,
    amount: u64,
) -> (DepositToInterestVault, Instruction) {
    let accounts = DepositToInterestVault {
        authority: *authority,
        state: *state,
        depositor: *depositor,
        depositor_token_account: *depositor_token_account,
        interest_distributor: *interest_distributor,
        interest_vault: *interest_vault,
        token_program: anchor_spl::token::ID,
    };

    let data = instruction::DepositToInterestVault { amount }.data();

    let instruction = Instruction {
        program_id: savings_program::ID,
        data,
        accounts: accounts.to_account_metas(None),
    };

    (accounts, instruction)
}

pub fn withdraw_from_interest_vault(
    authority: &Pubkey,
    destination_token_account: &Pubkey,
    state: &Pubkey,
    distributor: &Pubkey,
    interest_vault: &Pubkey,
    amount: u64,
) -> (WithdrawFromInterestVault, Instruction) {
    let accounts = WithdrawFromInterestVault {
        authority: *authority,
        destination_token_account: *destination_token_account,
        state: *state,
        interest_distributor: *distributor,
        interest_vault: *interest_vault,
        token_program: anchor_spl::token::ID,
    };

    let data = instruction::WithdrawFromInterestVault { amount }.data();

    let instruction = Instruction {
        program_id: savings_program::ID,
        data,
        accounts: accounts.to_account_metas(None),
    };

    (accounts, instruction)
}

pub fn user_create_vault(
    payer: &Pubkey,
    user: &Pubkey,
    mint: &Pubkey,
    distributor: &Pubkey,
    savings_manager: &Pubkey,
    savings_vault: &Pubkey,
) -> (UserCreateVault, Instruction) {
    let accounts = UserCreateVault {
        payer: *payer,
        user: *user,
        mint: *mint,
        interest_distributor: *distributor,
        savings_manager: *savings_manager,
        savings_vault: *savings_vault,
        system_program: system_program::ID,
        token_program: anchor_spl::token::ID,
        associated_token_program: anchor_spl::associated_token::ID,
    };

    let data = instruction::UserCreateVault {}.data();

    let instruction = Instruction {
        program_id: savings_program::ID,
        data,
        accounts: accounts.to_account_metas(None),
    };

    (accounts, instruction)
}

pub fn user_deposit(
    user: &Pubkey,
    user_token_account: &Pubkey,
    savings_manager: &Pubkey,
    savings_vault: &Pubkey,
    amount: u64,
) -> (UserDeposit, Instruction) {
    let accounts = UserDeposit {
        user: *user,
        user_token_account: *user_token_account,
        savings_manager: *savings_manager,
        savings_vault: *savings_vault,
        token_program: anchor_spl::token::ID,
    };

    let data = instruction::UserDeposit { amount }.data();

    let instruction = Instruction {
        program_id: savings_program::ID,
        data,
        accounts: accounts.to_account_metas(None),
    };

    (accounts, instruction)
}

pub fn user_withdraw(
    user: &Pubkey,
    savings_manager: &Pubkey,
    savings_vault: &Pubkey,
    destination_token_account: &Pubkey,
    amount: u64,
) -> (UserWithdraw, Instruction) {
    let accounts = UserWithdraw {
        user: *user,
        savings_manager: *savings_manager,
        savings_vault: *savings_vault,
        destination_token_account: *destination_token_account,
        token_program: anchor_spl::token::ID,
    };

    let data = instruction::UserWithdraw { amount }.data();

    let instruction = Instruction {
        program_id: savings_program::ID,
        data,
        accounts: accounts.to_account_metas(None),
    };

    (accounts, instruction)
}

pub fn deposit_interest(
    user: &Pubkey,
    user_savings_manager: &Pubkey,
    user_savings_vault: &Pubkey,
    interest_distributor: &Pubkey,
    interest_vault: &Pubkey,
) -> (DepositInterestToUser, Instruction) {
    let accounts = DepositInterestToUser {
        user: *user,
        user_savings_manager: *user_savings_manager,
        user_savings_vault: *user_savings_vault,
        interest_distributor: *interest_distributor,
        interest_vault: *interest_vault,
        token_program: anchor_spl::token::ID,
    };

    let data = instruction::DepositInterest {}.data();

    let instruction = Instruction {
        program_id: savings_program::ID,
        data,
        accounts: accounts.to_account_metas(None),
    };

    (accounts, instruction)
}
