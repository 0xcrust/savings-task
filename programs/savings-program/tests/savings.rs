#![allow(dead_code)]
mod helpers;

use anchor_spl::token::TokenAccount;
use helpers::{context, pda, program_test, utils};

use savings_program::{InterestDistributor, SavingsManager, State};
use solana_program_test::tokio;
use solana_sdk::clock::Clock;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_instruction;

#[tokio::test]
async fn test_all_actions() {
    let mut ctx = program_test().start_with_context().await;
    let mint = Keypair::new();
    let mint_authority = Keypair::new();

    // Initialize a test token mint.
    let create_mint =
        utils::create_token_mint(&mut ctx, &mint, &mint_authority.pubkey(), 0).unwrap();
    utils::send_and_confirm_tx(&mut ctx, create_mint, Some(vec![&mint]))
        .await
        .unwrap();

    let admin = Keypair::new();
    let state = Keypair::new();

    // Transfer 100 sol to the admin account so it can pay for transactions.
    let transfer_ix =
        system_instruction::transfer(&ctx.payer.pubkey(), &admin.pubkey(), 100_000_000_000);
    utils::send_and_confirm_tx(&mut ctx, vec![transfer_ix], None)
        .await
        .unwrap();

    // Create an associated-token-account for the admin to spend from.
    let (admin_ata, create_ata_ix) = utils::create_associated_token_account(
        &ctx.payer.pubkey(),
        &admin.pubkey(),
        &mint.pubkey(),
    );
    utils::send_and_confirm_tx(&mut ctx, vec![create_ata_ix], None)
        .await
        .unwrap();

    // Mint 100 tokens to the admin's ata.
    let mint_to_admin =
        utils::mint_tokens(&mint.pubkey(), &admin_ata, &mint_authority.pubkey(), 100).unwrap();

    utils::send_and_confirm_tx(&mut ctx, vec![mint_to_admin], Some(vec![&mint_authority]))
        .await
        .unwrap();

    // Initialize an application state.
    let ctx = context::TestContext::initialize_state(ctx, &admin, &state)
        .await
        .unwrap();
    let state_account = ctx
        .get_deserialized_account::<State>(&state.pubkey())
        .await
        .unwrap();
    assert_eq!(state_account.authority, admin.pubkey());

    // Register an interest-vault for a particular mint, allowing users to save tokens of that mint.
    ctx.create_interest_vault(&mint.pubkey()).await.unwrap();
    let (distributor, d_bump) =
        pda::derive_interest_distributor_pda(&state.pubkey(), &mint.pubkey());
    let interest_vault = pda::derive_interest_vault_ata(&mint.pubkey(), &distributor);

    let distributor_account = ctx
        .get_deserialized_account::<InterestDistributor>(&distributor)
        .await
        .unwrap();
    assert_eq!(distributor_account.bump, d_bump);
    assert_eq!(distributor_account.mint, mint.pubkey());
    assert_eq!(distributor_account.state, state.pubkey());

    // Admin: Deposit tokens to interest vaults for use in paying off interest.
    ctx.deposit_to_interest_vault(&mint.pubkey(), &admin, &admin_ata, 80)
        .await
        .unwrap();

    let vault_account = ctx
        .get_deserialized_account::<TokenAccount>(&interest_vault)
        .await
        .unwrap();
    assert!(vault_account.amount == 80);

    // Admin: Test withdrawing tokens from the interest vault back to the admin ata.
    ctx.withdraw_from_interest_vault(&mint.pubkey(), &admin_ata, 30)
        .await
        .unwrap();
    let vault_account = ctx
        .get_deserialized_account::<TokenAccount>(&interest_vault)
        .await
        .unwrap();
    assert!(vault_account.amount == 50);

    let admin_ata_account = ctx
        .get_deserialized_account::<TokenAccount>(&admin_ata)
        .await
        .unwrap();
    assert!(admin_ata_account.amount == 50);

    let user = Keypair::new();
    // Transfer 100 sol to the user account so it can pay for transactions.
    let transfer_ix = system_instruction::transfer(
        &ctx.ctx.borrow().payer.pubkey(),
        &user.pubkey(),
        100_000_000_000,
    );
    ctx.send_and_confirm_tx(vec![transfer_ix], None)
        .await
        .unwrap();

    // Create an associated-token-account for the user to spend from.
    let (user_ata, create_ata_ix) = utils::create_associated_token_account(
        &ctx.ctx.borrow().payer.pubkey(),
        &user.pubkey(),
        &mint.pubkey(),
    );
    ctx.send_and_confirm_tx(vec![create_ata_ix], None)
        .await
        .unwrap();

    // Mint 1000 tokens to the user's ata.
    let mint_to =
        utils::mint_tokens(&mint.pubkey(), &user_ata, &mint_authority.pubkey(), 1000).unwrap();
    ctx.send_and_confirm_tx(vec![mint_to], Some(vec![&mint_authority]))
        .await
        .unwrap();

    // User: Create a vault for saving.
    let expected_timestamp = ctx
        .ctx
        .borrow_mut()
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .unwrap()
        .unix_timestamp;
    ctx.user_create_vault(&user, &mint.pubkey()).await.unwrap();
    let (savings_manager, sm_bump) = pda::derive_savings_manager_pda(&user.pubkey(), &distributor);
    let savings_vault = pda::derive_savings_vault_ata(&mint.pubkey(), &savings_manager);

    let sm_account = ctx
        .get_deserialized_account::<SavingsManager>(&savings_manager)
        .await
        .unwrap();
    assert_eq!(sm_account.user, user.pubkey());
    assert_eq!(sm_account.mint, mint.pubkey());
    assert_eq!(sm_account.bump, sm_bump);
    assert_eq!(sm_account.distributor, distributor);
    assert_eq!(sm_account.last_interest_deposit_ts, expected_timestamp);
    println!("last timestamp: {}", expected_timestamp);

    // User: Deposit 900 tokens to the interest vault.
    ctx.user_deposit(&user, &mint.pubkey(), &user_ata, 900)
        .await
        .unwrap();

    let vault_account = ctx
        .get_deserialized_account::<TokenAccount>(&savings_vault)
        .await
        .unwrap();
    assert!(vault_account.amount == 900);

    // User: Withdraw 400 tokens back into user-ata.
    ctx.user_withdraw(&user, &mint.pubkey(), &user_ata, 400)
        .await
        .unwrap();
    let user_ata_account = ctx
        .get_deserialized_account::<TokenAccount>(&user_ata)
        .await
        .unwrap();
    assert!(user_ata_account.amount == 500);

    let vault_account = ctx
        .get_deserialized_account::<TokenAccount>(&savings_vault)
        .await
        .unwrap();
    assert!(vault_account.amount == 500);

    // Attempting to deposit interest now should error:
    let result = ctx.deposit_interest(&user.pubkey(), &mint.pubkey()).await;
    assert!(result.is_err());

    // Fast-forward time to a month after. A month is (30 * 24 * 60 * 60) seconds so we add that
    // to create our artificial clock.

    // Commented out because `set_sysvar()` does not seem to have the desired effect and unix-timestamp
    // remains unchanged.
    /*
    let mut clock = ctx.ctx.borrow_mut().banks_client.get_sysvar::<Clock>().await.unwrap();
    let forwarded_time = clock.unix_timestamp;
    clock.unix_timestamp = clock.unix_timestamp + (30 * 24 * 60 * 60);

    ctx.ctx.borrow_mut().set_sysvar(&clock);
    ctx.deposit_interest(&user.pubkey(), &mint.pubkey())
        .await
        .unwrap();

    // Interest payout is expected to be 1% of the user's current savings balance(500).
    let user_ata_account = ctx
        .get_deserialized_account::<TokenAccount>(&user_ata)
        .await
        .unwrap();
    assert!(user_ata_account.amount == 505);

    let vault_account = ctx
        .get_deserialized_account::<TokenAccount>(&savings_vault)
        .await
        .unwrap();
    assert!(vault_account.amount == 495);

    let savings_manager = ctx
        .get_deserialized_account::<SavingsManager>(&savings_manager)
        .await
        .unwrap();
    assert!(savings_manager.last_interest_deposit_ts == forwarded_time); */
}
