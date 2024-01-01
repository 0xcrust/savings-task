#![allow(clippy::result_large_err)]

use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount, Transfer};

declare_id!("BYDhC79wks4E3P5Fi5Ez4oKwS8fM1PQFVnRQLZsa4YdP");

pub const SAVINGS_MANAGER_SEED_PREFIX: &[u8] = b"savings-manager";
pub const INTEREST_DISTRIBUTOR_SEED_PREFIX: &[u8] = b"interest-distributor";

pub const INTEREST_PERCENTAGE: u64 = 1;
pub const SECONDS_IN_MONTHS: i64 = 30 * 24 * 60 * 60;

pub fn current_time() -> Result<i64> {
    Ok(anchor_lang::solana_program::sysvar::clock::Clock::get()?.unix_timestamp)
}

#[program]
pub mod savings_program {
    use super::*;

    //////////////////////////////////////////////////////////////////////////////////////
    // ADMIN INSTRUCTIONS.
    //////////////////////////////////////////////////////////////////////////////////////

    // Initialize a state account. This state will be responsible for distributing interest tokens
    // to (**ONLY**) user vaults that are registered to it.
    pub fn initialize_state(ctx: Context<InitializeState>, authority: Pubkey) -> Result<()> {
        ctx.accounts.state.authority = authority;
        Ok(())
    }

    // Register an `interest-distributor` for a mint and create an accompanying `interest-vault`.
    // Interest tokens are paid out from the vault permissionlessly at the bequest of the distributor.
    pub fn create_interest_vault(ctx: Context<CreateInterestVaultForMint>) -> Result<()> {
        let distributor = &mut ctx.accounts.interest_distributor;
        distributor.state = ctx.accounts.state.key();
        distributor.mint = ctx.accounts.mint.key();
        distributor.bump = *ctx.bumps.get("interest_distributor").unwrap();

        Ok(())
    }

    // Top up the amount of tokens in the interest vault.
    pub fn deposit_to_interest_vault(
        ctx: Context<DepositToInterestVault>,
        amount: u64,
    ) -> Result<()> {
        anchor_spl::token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.depositor_token_account.to_account_info(),
                    to: ctx.accounts.interest_vault.to_account_info(),
                    authority: ctx.accounts.depositor.to_account_info(),
                },
            ),
            amount,
        )?;

        Ok(())
    }

    // Withdraw some amount of tokens from the interest vault.
    pub fn withdraw_from_interest_vault(
        ctx: Context<WithdrawFromInterestVault>,
        amount: u64,
    ) -> Result<()> {
        let state_key = ctx.accounts.interest_distributor.state;
        let mint_key = ctx.accounts.interest_distributor.mint;
        let distributor_seeds = [
            INTEREST_DISTRIBUTOR_SEED_PREFIX,
            state_key.as_ref(),
            mint_key.as_ref(),
            &[ctx.accounts.interest_distributor.bump],
        ];

        anchor_spl::token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.interest_vault.to_account_info(),
                    to: ctx.accounts.destination_token_account.to_account_info(),
                    authority: ctx.accounts.interest_distributor.to_account_info(),
                },
            )
            .with_signer(&[&distributor_seeds[..]]),
            amount,
        )?;

        Ok(())
    }

    //////////////////////////////////////////////////////////////////////////////////////
    // USER INSTRUCTIONS.
    //////////////////////////////////////////////////////////////////////////////////////

    // Create a savings vault for a particular user, registered to an existing interest distributor.
    pub fn user_create_vault(ctx: Context<UserCreateVault>) -> Result<()> {
        let manager = &mut ctx.accounts.savings_manager;
        manager.user = ctx.accounts.user.key();
        manager.mint = ctx.accounts.mint.key();
        manager.distributor = ctx.accounts.interest_distributor.key();
        manager.last_interest_deposit_ts = current_time()?;
        manager.bump = *ctx.bumps.get("savings_manager").unwrap();
        Ok(())
    }

    // Deposit tokens to a user's savings vault.
    pub fn user_deposit(ctx: Context<UserDeposit>, amount: u64) -> Result<()> {
        anchor_spl::token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.savings_vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
        )?;

        Ok(())
    }

    // Withdraw tokens from a user's savings vault.
    pub fn user_withdraw(ctx: Context<UserWithdraw>, amount: u64) -> Result<()> {
        let manager_seeds = &[
            SAVINGS_MANAGER_SEED_PREFIX,
            ctx.accounts.savings_manager.user.as_ref(),
            ctx.accounts.savings_manager.distributor.as_ref(),
            &[ctx.accounts.savings_manager.bump],
        ];

        anchor_spl::token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.savings_vault.to_account_info(),
                    to: ctx.accounts.destination_token_account.to_account_info(),
                    authority: ctx.accounts.savings_manager.to_account_info(),
                },
            )
            .with_signer(&[&manager_seeds[..]]),
            amount,
        )?;

        Ok(())
    }

    //////////////////////////////////////////////////////////////////////////////////////
    // PERMISSIONLESS INSTRUCTIONS.
    //////////////////////////////////////////////////////////////////////////////////////

    // Permissionless instruction, intended to be called by a crank to deposit 1% interest
    // to a user's savings account every month.
    pub fn deposit_interest(ctx: Context<DepositInterestToUser>) -> Result<()> {
        let current_time = current_time()?;
        let seconds_elapsed = current_time
            .checked_sub(ctx.accounts.user_savings_manager.last_interest_deposit_ts)
            .unwrap();

        if seconds_elapsed < SECONDS_IN_MONTHS {
            msg!(
                "Crank Error: Last deposit timestamp: {}. Current timestamp: {}",
                ctx.accounts.user_savings_manager.last_interest_deposit_ts,
                current_time
            );
            return Err(SavingsError::CrankTurnedTooSoon.into());
        }

        let vault = &ctx.accounts.user_savings_vault;
        let interest_amount = INTEREST_PERCENTAGE
            .checked_mul(vault.amount)
            .unwrap()
            .checked_div(100)
            .unwrap();

        if ctx.accounts.interest_vault.amount < interest_amount {
            return Err(SavingsError::InadequateFunds.into());
        }

        let state_key = ctx.accounts.interest_distributor.state;
        let mint_key = ctx.accounts.interest_distributor.mint;
        let distributor_seeds = [
            INTEREST_DISTRIBUTOR_SEED_PREFIX,
            state_key.as_ref(),
            mint_key.as_ref(),
            &[ctx.accounts.interest_distributor.bump],
        ];
        anchor_spl::token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.interest_vault.to_account_info(),
                    to: ctx.accounts.user_savings_vault.to_account_info(),
                    authority: ctx.accounts.interest_distributor.to_account_info(),
                },
            )
            .with_signer(&[&distributor_seeds[..]]),
            interest_amount,
        )?;

        // Reset the last-interest-deposit-timestamp.
        ctx.accounts.user_savings_manager.last_interest_deposit_ts = current_time;

        Ok(())
    }

    // Similar to `deposit_interest`, but can deposit to multiple users in the same instruction.
    pub fn deposit_interest_multiple<'info>(
        ctx: Context<'_, '_, '_, 'info, DepositInterestToMultipleUsers<'info>>,
    ) -> Result<()> {
        // This instruction requires that the requisite accounts for each user be passed in trios
        // from `ctx.remaining_accounts`:
        // 1. The user's wallet,
        // 2. The user's savings-manager account, and
        // 3. The user's savings-vault account.

        let distributor = &mut ctx.accounts.interest_distributor;

        if ctx.remaining_accounts.len() < 3 {
            return Err(SavingsError::ZeroRecipientsForInterestDeposit.into());
        }

        for chunk in ctx.remaining_accounts.chunks_exact(3) {
            let user_wallet = &chunk[0];
            let unchecked_savings_manager = &chunk[1];
            let unchecked_savings_vault = &chunk[2];

            // Check that invariants are held for the unvalidated savings-manager account:
            let (derived_savings_manager, _) = Pubkey::find_program_address(
                &[
                    SAVINGS_MANAGER_SEED_PREFIX,
                    user_wallet.key().as_ref(),
                    distributor.key().as_ref(),
                ],
                &crate::ID,
            );
            require_keys_eq!(derived_savings_manager, *unchecked_savings_manager.key);
            let mut savings_manager =
                Account::<'info, SavingsManager>::try_from(unchecked_savings_manager)?;

            // Check that invariants are held for the unvalidated savings-vault account.
            let savings_vault = Account::<'info, TokenAccount>::try_from(&chunk[2])?;
            let associated_token_address =
                anchor_spl::associated_token::get_associated_token_address(
                    &savings_manager.key(),
                    &savings_manager.mint,
                );
            require_keys_eq!(associated_token_address, *unchecked_savings_vault.key);
            require_keys_eq!(savings_vault.owner, savings_manager.key());

            // Perform the interest transfer.
            let current_time = current_time()?;
            let seconds_elapsed = current_time
                .checked_sub(savings_manager.last_interest_deposit_ts)
                .unwrap();

            if seconds_elapsed < SECONDS_IN_MONTHS {
                msg!(
                    "Crank Error: Last deposit timestamp: {}. Current timestamp: {}",
                    savings_manager.last_interest_deposit_ts,
                    current_time
                );
                return Err(SavingsError::CrankTurnedTooSoon.into());
            }

            let interest_amount = INTEREST_PERCENTAGE
                .checked_mul(savings_vault.amount)
                .unwrap()
                .checked_div(100)
                .unwrap();

            if ctx.accounts.interest_vault.amount < interest_amount {
                return Err(SavingsError::InadequateFunds.into());
            }

            let state_key = distributor.state;
            let mint_key = distributor.mint;
            let distributor_seeds = [
                INTEREST_DISTRIBUTOR_SEED_PREFIX,
                state_key.as_ref(),
                mint_key.as_ref(),
                &[distributor.bump],
            ];

            anchor_spl::token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.interest_vault.to_account_info(),
                        to: savings_vault.to_account_info(),
                        authority: distributor.to_account_info(),
                    },
                )
                .with_signer(&[&distributor_seeds[..]]),
                interest_amount,
            )?;

            // Reset the last-interest-deposit-timestamp.
            savings_manager.last_interest_deposit_ts = current_time;
        }

        Ok(())
    }
}

//////////////////////////////////////////
// CONTEXT FOR USER INSTRUCTIONS:
/////////////////////////////////////////

#[derive(Accounts)]
pub struct UserCreateVault<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub user: Signer<'info>,
    pub mint: Account<'info, Mint>,
    // This account must exist as a user cannot create a vault account
    // for an unregistered mint.
    #[account(has_one = mint)]
    pub interest_distributor: Account<'info, InterestDistributor>,
    #[account(
        init,
        seeds = [SAVINGS_MANAGER_SEED_PREFIX, user.key().as_ref(), interest_distributor.key().as_ref()],
        bump,
        payer = payer,
        space = SavingsManager::SPACE,
    )]
    pub savings_manager: Account<'info, SavingsManager>,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = savings_manager
    )]
    pub savings_vault: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct UserDeposit<'info> {
    pub user: Signer<'info>,
    #[account(has_one = user)]
    pub savings_manager: Account<'info, SavingsManager>,
    /// CHECK: Checked by SPL-token Transfer Instruction.
    #[account(mut)]
    pub user_token_account: UncheckedAccount<'info>,
    #[account(
        mut,
        associated_token::mint = savings_manager.mint,
        associated_token::authority = savings_manager
    )]
    pub savings_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct UserWithdraw<'info> {
    pub user: Signer<'info>,
    #[account(has_one = user)]
    pub savings_manager: Account<'info, SavingsManager>,
    #[account(
        mut,
        associated_token::mint = savings_manager.mint,
        associated_token::authority = savings_manager,
        constraint = savings_vault.amount >= amount @ SavingsError::InadequateFunds
    )]
    pub savings_vault: Account<'info, TokenAccount>,
    /// CHECK: Checked by SPL-token Transfer Instruction.
    #[account(mut)]
    pub destination_token_account: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
}

//////////////////////////////////////////
// CONTEXT FOR ADMIN INSTRUCTIONS:
/////////////////////////////////////////

#[derive(Accounts)]
pub struct InitializeState<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,
    #[account(
        init,
        payer = initializer,
        space = 8 + 32,
        //space = State::SPACE
    )]
    pub state: Account<'info, State>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateInterestVaultForMint<'info> {
    pub authority: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(has_one = authority)]
    pub state: Account<'info, State>,
    pub mint: Account<'info, Mint>,
    #[account(
        init,
        seeds = [INTEREST_DISTRIBUTOR_SEED_PREFIX, state.key().as_ref(), mint.key().as_ref()],
        bump,
        payer = payer,
        space = InterestDistributor::SPACE,
    )]
    pub interest_distributor: Account<'info, InterestDistributor>,
    #[account(
        init,
        associated_token::mint = mint,
        associated_token::authority = interest_distributor,
        payer = payer,
    )]
    pub interest_vault: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct DepositToInterestVault<'info> {
    pub authority: Signer<'info>,
    #[account(has_one = authority)]
    pub state: Account<'info, State>,
    pub depositor: Signer<'info>,
    /// CHECK: Checked by SPL-token Transfer Instruction.
    #[account(mut)]
    pub depositor_token_account: UncheckedAccount<'info>,
    #[account(has_one = state)]
    pub interest_distributor: Account<'info, InterestDistributor>,
    #[account(
        mut,
        associated_token::mint = interest_distributor.mint,
        associated_token::authority = interest_distributor,
    )]
    pub interest_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct WithdrawFromInterestVault<'info> {
    pub authority: Signer<'info>,
    /// CHECK: Checked by SPL-token Transfer Instruction.
    #[account(mut)]
    pub destination_token_account: UncheckedAccount<'info>,
    #[account(has_one = authority)]
    pub state: Account<'info, State>,
    #[account(has_one = state)]
    pub interest_distributor: Account<'info, InterestDistributor>,
    #[account(
        mut,
        associated_token::mint = interest_distributor.mint,
        associated_token::authority = interest_distributor,
    )]
    pub interest_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

///////////////////////////////////////////
// CONTEXT FOR CRANK INSTRUCTION:
/////////////////////////////////////////

#[derive(Accounts)]
pub struct DepositInterestToUser<'info> {
    /// CHECK: The user the crank is being called for.
    pub user: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [SAVINGS_MANAGER_SEED_PREFIX, user.key().as_ref(), interest_distributor.key().as_ref()],
        bump,
    )]
    pub user_savings_manager: Account<'info, SavingsManager>,
    #[account(
        mut,
        associated_token::mint = user_savings_manager.mint,
        associated_token::authority = user_savings_manager
    )]
    pub user_savings_vault: Account<'info, TokenAccount>,
    pub interest_distributor: Account<'info, InterestDistributor>,
    #[account(
        mut,
        associated_token::mint = interest_distributor.mint,
        associated_token::authority = interest_distributor,
    )]
    pub interest_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct DepositInterestToMultipleUsers<'info> {
    pub interest_distributor: Account<'info, InterestDistributor>,
    #[account(
        mut,
        associated_token::mint = interest_distributor.mint,
        associated_token::authority = interest_distributor,
    )]
    pub interest_vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[account]
/// The Application State.
pub struct State {
    /// The authority that's allowed to deposit and withdraw
    /// from the interest vault.
    pub authority: Pubkey,
}

impl State {
    pub const SPACE: usize = 8/*anchor account discriminator*/ + 32/*authority*/;
}

#[account]
/// This account is a PDA unique to a single (state, mint) pair, and is authority
/// of the vault from which interest tokens are paid.
pub struct InterestDistributor {
    /// The state's public key.
    pub state: Pubkey,
    /// The mint of the token account this distributor serves as authority for.
    pub mint: Pubkey,
    /// Bump of this account's PDA. Stored to avoid deriving it everytime a signature
    /// is required.
    pub bump: u8,
}

impl InterestDistributor {
    pub const SPACE: usize = 8 +   // anchor account discriminator
        32 +   // state
        32 +   // mint
        1; // bump
}

#[account]
/// Account holding information for a user-owned vault. This is a PDA unique to a
/// (user, interest-distributor) pair.
pub struct SavingsManager {
    /// The owner of the vault.
    pub user: Pubkey,
    /// The mint of the vault.
    pub mint: Pubkey,
    /// The distributor for this user's vault, responsible for disbursing its interest payments.
    pub distributor: Pubkey,
    /// The unix timestamp of the last interest deposit.
    pub last_interest_deposit_ts: i64,
    /// Bump of this account's PDA. Stored to avoid deriving it everytime a signature
    /// is required.
    pub bump: u8,
}

impl SavingsManager {
    pub const SPACE: usize = 8 +   // anchor account discriminator
        32 +   // user
        32 +   // mint
        32 +   // distributor
        8 +    // last_interest_deposit_ts
        1; // bump
}

#[error_code]
pub enum SavingsError {
    #[msg("not enough funds in vault token account")]
    InadequateFunds,
    #[msg("not enough time has elapsed since the last interest deposit")]
    CrankTurnedTooSoon,
    #[msg("did not specify any recipient for the interest transfer")]
    ZeroRecipientsForInterestDeposit,
}
