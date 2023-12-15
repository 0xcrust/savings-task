use super::Result;
use anchor_spl::token::spl_token;
use anchor_spl::token::Mint;
use solana_program_test::ProgramTestContext;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_instruction;
use solana_sdk::sysvar::rent::Rent;
use solana_sdk::transaction::Transaction;

pub fn create_token_mint(
    ctx: &mut ProgramTestContext,
    mint: &Keypair,
    authority: &Pubkey,
    decimals: u8,
) -> Result<Vec<Instruction>> {
    let create_account = system_instruction::create_account(
        &ctx.payer.pubkey(),
        &mint.pubkey(),
        Rent::default().minimum_balance(Mint::LEN),
        Mint::LEN as u64,
        &spl_token::id(),
    );
    let initialize_mint = spl_token::instruction::initialize_mint(
        &spl_token::id(),
        &mint.pubkey(),
        authority,
        None,
        decimals,
    )?;

    Ok(vec![create_account, initialize_mint])
}

pub fn create_associated_token_account(
    payer: &Pubkey,
    owner: &Pubkey,
    token_mint: &Pubkey,
) -> (Pubkey, Instruction) {
    let ata = spl_associated_token_account::get_associated_token_address(owner, token_mint);
    let ix = spl_associated_token_account::instruction::create_associated_token_account(
        payer,
        owner,
        token_mint,
        &anchor_spl::token::ID,
    );
    (ata, ix)
}

pub fn mint_tokens(
    mint: &Pubkey,
    token_account: &Pubkey,
    mint_authority: &Pubkey,
    amount: u64,
) -> Result<Instruction> {
    Ok(spl_token::instruction::mint_to(
        &anchor_spl::token::ID,
        mint,
        token_account,
        &mint_authority,
        &[],
        amount,
    )?)
}

pub async fn send_and_confirm_tx(
    ctx: &mut ProgramTestContext,
    ix: Vec<Instruction>,
    signers: Option<Vec<&Keypair>>,
) -> Result<()> {
    let mut signers = signers.unwrap_or_default();
    signers.push(&ctx.payer);

    let tx = Transaction::new_signed_with_payer(
        &ix,
        Some(&ctx.payer.pubkey()),
        &signers,
        ctx.last_blockhash,
    );

    ctx.banks_client.process_transaction(tx).await?;

    Ok(())
}
