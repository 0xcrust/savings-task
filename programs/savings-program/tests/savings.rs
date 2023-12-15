#![allow(dead_code)]
mod helpers;
use std::borrow::BorrowMut;

use anchor_lang::AccountDeserialize;
use helpers::{context, program_test, utils};

use solana_program_test::tokio;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_instruction;

#[tokio::test]
async fn test_all_actions() {
    let mut ctx = program_test().start_with_context().await;
    let mint = Keypair::new();
    let mint_authority = Keypair::new();
    let create_mint =
        utils::create_token_mint(&mut ctx, &mint, &mint_authority.pubkey(), 0).unwrap();
    utils::send_and_confirm_tx(&mut ctx, create_mint, Some(vec![&mint]))
        .await
        .unwrap();

    let admin = Keypair::new();
    let state = Keypair::new();

    let transfer_ix =
        system_instruction::transfer(&ctx.payer.pubkey(), &admin.pubkey(), 100_000_000_000);
    utils::send_and_confirm_tx(&mut ctx, vec![transfer_ix], None)
        .await
        .unwrap();

    let (admin_ata, create_ata_ix) = utils::create_associated_token_account(
        &ctx.payer.pubkey(),
        &admin.pubkey(),
        &mint.pubkey(),
    );

    utils::send_and_confirm_tx(&mut ctx, vec![create_ata_ix], None)
        .await
        .unwrap();

    let token_account = ctx
        .borrow_mut()
        .banks_client
        .get_account(admin_ata)
        .await
        .unwrap()
        .unwrap();
    let token_account =
        anchor_spl::token::TokenAccount::try_deserialize(&mut token_account.data.as_ref()).unwrap();
    assert!(token_account.mint == mint.pubkey());

    std::thread::sleep(std::time::Duration::from_secs(2));
    let mint_to_admin =
        utils::mint_tokens(&mint.pubkey(), &admin_ata, &mint_authority.pubkey(), 100).unwrap();

    utils::send_and_confirm_tx(&mut ctx, vec![mint_to_admin], Some(vec![&mint_authority]))
        .await
        .unwrap();

    let ctx = context::TestContext::initialize_state(ctx, &admin, &state)
        .await
        .unwrap();
    ctx.create_interest_vault(&mint.pubkey()).await.unwrap();

    ctx.deposit_to_interest_vault(&mint.pubkey(), &admin, &admin_ata, 80)
        .await
        .unwrap();

    ctx.withdraw_from_interest_vault(&mint.pubkey(), &admin_ata, 30)
        .await
        .unwrap();
}
