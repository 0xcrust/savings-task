pub mod context;
pub mod instructions;
pub mod pda;
pub mod utils;

use solana_program_test::{processor, BanksClientError, ProgramTest};
use solana_sdk::program_error::ProgramError;

pub type Result<T> = std::result::Result<T, Error>;

pub fn program_test() -> ProgramTest {
    let mut program_test = ProgramTest::new(
        "savings_program",
        savings_program::ID,
        processor!(savings_program::entry),
    );
    program_test.prefer_bpf(false);
    program_test
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Program(#[from] ProgramError),
    #[error(transparent)]
    Client(#[from] BanksClientError),
    #[error(transparent)]
    Lang(#[from] anchor_lang::error::Error),
    #[error("Tried to fetch a non-existent account")]
    AccountNotFound,
    #[error(transparent)]
    Signature(#[from] solana_sdk::signature::SignerError),
}
