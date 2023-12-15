use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount, Transfer};

declare_id!("2vcQGZqkPMf5qL96ddFTgXuN9umpgVE7UKLNeaQUWMVU");

pub const MANAGER_SEED_PREFIX: &[u8] = b"vault-manager";
pub const INTEREST_DISTRIBUTOR_SEED_PREFIX: &[u8] = b"interest-distributor";
pub const INTEREST_VAULT_SEED_PREFIX: &[u8] = b"interest-vault";

pub const INTEREST_PERCENTAGE: u64 = 1;
pub const SECONDS_IN_MONTHS: i64 = 30 * 24 * 60 * 60;

pub fn current_time() -> Result<i64> {
    Ok(anchor_lang::solana_program::sysvar::clock::Clock::get()?.unix_timestamp)
}

#[program]
pub mod gfx_task {
    use super::*;

    //////////////////////////////////////////////////////////////////////////////////////
    // ADMIN INSTRUCTIONS
    //////////////////////////////////////////////////////////////////////////////////////

    // Initializes a state account. This state will be responsible for distributing interest tokens
    // to (**ONLY**) user vaults that are registered to it.
    pub fn initialize_state(ctx: Context<InitializeState>, authority: Pubkey) -> Result<()> {
        ctx.accounts.state.authority = authority;
        Ok(())
    }

    // Registers an `interest-distributor` for a mint and creates an accompanying `interest-vault`.
    // Interest tokens are paid out from the vault permissionlessly at the bequest of the distributor.
    pub fn create_interest_vault(ctx: Context<CreateInterestVaultForMint>) -> Result<()> {
        let distributor = &mut ctx.accounts.interest_distributor;
        distributor.state = ctx.accounts.state.key();
        distributor.mint = ctx.accounts.mint.key();
        distributor.bump = ctx.bumps.interest_distributor;

        Ok(())
    }

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
    // USER INSTRUCTIONS
    //////////////////////////////////////////////////////////////////////////////////////

    pub fn user_create_vault(ctx: Context<UserCreateVault>) -> Result<()> {
        let manager = &mut ctx.accounts.savings_manager;
        manager.user = ctx.accounts.user.key();
        manager.mint = ctx.accounts.mint.key();
        manager.distributor = ctx.accounts.interest_distributor.key();
        manager.last_interest_deposit_ts = current_time()?;
        manager.bump = ctx.bumps.savings_manager;
        Ok(())
    }

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

    pub fn user_withdraw(ctx: Context<UserWithdraw>, amount: u64) -> Result<()> {
        let manager_seeds = &[
            MANAGER_SEED_PREFIX,
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
    // PERMISSIONLESS INSTRUCTIONS
    //////////////////////////////////////////////////////////////////////////////////////

    pub fn deposit_interest(ctx: Context<DepositInterestToUser>) -> Result<()> {
        let current_time = current_time()?;
        let seconds_elapsed = current_time
            .checked_sub(ctx.accounts.user_savings_manager.last_interest_deposit_ts)
            .unwrap();

        if seconds_elapsed < SECONDS_IN_MONTHS {
            return Err(SavingsError::CrankTurnedTooSoon.into());
        }

        let vault = &ctx.accounts.user_savings_vault;
        let interest_amount = INTEREST_PERCENTAGE
            .checked_mul(vault.amount)
            .unwrap()
            .checked_div(100)
            .unwrap();

        anchor_spl::token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.interest_vault.to_account_info(),
                    to: ctx.accounts.user_savings_vault.to_account_info(),
                    authority: ctx.accounts.interest_distributor.to_account_info(),
                },
            ),
            interest_amount,
        )?;

        // Reset the last-interest-deposit-timestamp.
        ctx.accounts.user_savings_manager.last_interest_deposit_ts = current_time;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct UserCreateVault<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub user: Signer<'info>,
    pub mint: Account<'info, Mint>,
    // This account must exist as a user cannot create a vault account
    // for an unregistered mint.
    #[account(mut, has_one = mint)]
    pub interest_distributor: Account<'info, InterestDistributor>,
    #[account(
        init,
        seeds = [MANAGER_SEED_PREFIX, user.key().as_ref(), interest_distributor.key().as_ref()],
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
// Context for Admin Instructions.
/////////////////////////////////////////

#[derive(Accounts)]
pub struct InitializeState<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,
    #[account(
        init,
        payer = initializer,
        space = State::SPACE
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

#[derive(Accounts)]
pub struct DepositInterestToUser<'info> {
    /// CHECK: The user the crank is being called for.
    pub user: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [MANAGER_SEED_PREFIX, user.key().as_ref(), interest_distributor.key().as_ref()],
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

#[account]
/// The Application State.
pub struct State {
    /// The authority that's allowed to deposit and withdraw
    /// from the interest vault.
    pub authority: Pubkey,
}

impl State {
    pub const SPACE: usize = 8/*discriminator*/ + 32/*authority*/;
}

#[account]
/// This account is unique to a single (state, mint) pair, and is responsible
/// for the vault from which interest tokens are paid.
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
    pub const SPACE: usize = 8 /*discriminator*/ +
        32 +    /* state*/
        32 +    /* mint*/
        1; /*bump*/
}

#[account]
/// Account holding information for a user-owned vault.
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
    pub const SPACE: usize = 8 /*discriminator*/ +
        32 +    /*user*/
        32 +    /*mint*/
        32 +    /*state*/
        8 +     /*last_interest_deposit_ts*/
        1; /*bump*/
}

#[error_code]
pub enum SavingsError {
    #[msg("not enough funds in vault token account")]
    InadequateFunds,
    #[msg("not enough time has elapsed since the last interest deposit")]
    CrankTurnedTooSoon,
}
