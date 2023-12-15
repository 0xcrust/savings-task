use super::Result;
use super::{instructions::*, pda};
use solana_program_test::ProgramTestContext;
use solana_sdk::account::Account;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::cell::RefCell;

pub struct TestContext {
    pub ctx: RefCell<ProgramTestContext>,
    pub admin: Keypair,
    pub state: Pubkey,
}

pub fn clone_keypair(keypair: &Keypair) -> Keypair {
    Keypair::from_bytes(&keypair.to_bytes()).unwrap()
}

impl TestContext {
    pub async fn initialize_state(
        ctx: ProgramTestContext,
        admin: &Keypair,
        state: &Keypair,
    ) -> Result<Self> {
        let (_, instruction) = initialize_state(&admin.pubkey(), &state.pubkey(), None);

        let ctx = TestContext {
            ctx: RefCell::new(ctx),
            admin: clone_keypair(admin),
            state: state.pubkey(),
        };

        ctx.send_and_confirm_tx(vec![instruction], Some(vec![state, admin]))
            .await?;

        Ok(ctx)
    }

    pub async fn create_interest_vault(&self, mint: &Pubkey) -> Result<()> {
        let distributor = pda::derive_interest_distributor_pda(&self.state, mint).0;
        let vault = pda::derive_interest_vault_ata(mint, &distributor);

        let (_, instruction) = create_interest_vault(
            &self.ctx.borrow().payer.pubkey(),
            &self.admin.pubkey(),
            &self.state,
            mint,
            &distributor,
            &vault,
        );

        self.send_and_confirm_tx(vec![instruction], Some(vec![&self.admin]))
            .await?;

        Ok(())
    }

    pub async fn deposit_to_interest_vault(
        &self,
        mint: &Pubkey,
        depositor: &Keypair,
        token_account_address: &Pubkey,
        amount: u64,
    ) -> Result<()> {
        let distributor = pda::derive_interest_distributor_pda(&self.state, mint).0;
        let vault = pda::derive_interest_vault_ata(mint, &distributor);

        let (_, instruction) = deposit_to_interest_vault(
            &self.admin.pubkey(),
            &self.state,
            &depositor.pubkey(),
            token_account_address,
            &distributor,
            &vault,
            amount,
        );

        self.send_and_confirm_tx(vec![instruction], Some(vec![&self.admin, depositor]))
            .await?;
        Ok(())
    }

    pub async fn withdraw_from_interest_vault(
        &self,
        mint: &Pubkey,
        destination_account: &Pubkey,
        amount: u64,
    ) -> Result<()> {
        let distributor = pda::derive_interest_distributor_pda(&self.state, mint).0;
        let vault = pda::derive_interest_vault_ata(mint, &distributor);

        let (_, instruction) = withdraw_from_interest_vault(
            &self.admin.pubkey(),
            destination_account,
            &self.state,
            &distributor,
            &vault,
            amount,
        );

        self.send_and_confirm_tx(vec![instruction], Some(vec![&self.admin]))
            .await?;
        Ok(())
    }

    pub async fn user_create_vault(&self, user: &Keypair, mint: &Pubkey) -> Result<()> {
        let distributor = pda::derive_interest_distributor_pda(&self.state, mint).0;
        let manager = pda::derive_savings_manager_pda(&user.pubkey(), &distributor).0;
        let vault = pda::derive_savings_vault_ata(mint, &manager);

        let (_, instruction) = user_create_vault(
            &self.ctx.borrow().payer.pubkey(),
            &user.pubkey(),
            mint,
            &distributor,
            &manager,
            &vault,
        );

        self.send_and_confirm_tx(vec![instruction], Some(vec![user]))
            .await?;
        Ok(())
    }

    pub async fn user_deposit(
        &self,
        user: &Keypair,
        mint: &Pubkey,
        token_account: &Pubkey,
        amount: u64,
    ) -> Result<()> {
        let distributor = pda::derive_interest_distributor_pda(&self.state, mint).0;
        let manager = pda::derive_savings_manager_pda(&user.pubkey(), &distributor).0;
        let vault = pda::derive_savings_vault_ata(mint, &manager);

        let (_, instruction) =
            user_deposit(&user.pubkey(), token_account, &manager, &vault, amount);

        self.send_and_confirm_tx(vec![instruction], Some(vec![user]))
            .await?;
        Ok(())
    }

    pub async fn user_withdraw(
        &self,
        user: &Keypair,
        mint: &Pubkey,
        token_account: &Pubkey,
        amount: u64,
    ) -> Result<()> {
        let distributor = pda::derive_interest_distributor_pda(&self.state, mint).0;
        let manager = pda::derive_savings_manager_pda(&user.pubkey(), &distributor).0;
        let vault = pda::derive_savings_vault_ata(mint, &manager);

        let (_, instruction) =
            user_withdraw(&user.pubkey(), &manager, &vault, token_account, amount);

        self.send_and_confirm_tx(vec![instruction], Some(vec![user]))
            .await?;
        Ok(())
    }

    pub async fn deposit_interest(&self, user: &Pubkey, mint: &Pubkey) -> Result<()> {
        let distributor = pda::derive_interest_distributor_pda(&self.state, mint).0;
        let interest_vault = pda::derive_interest_vault_ata(mint, &distributor);
        let manager = pda::derive_savings_manager_pda(user, &distributor).0;
        let savings_vault = pda::derive_savings_vault_ata(mint, &manager);

        let (_, instruction) = deposit_interest(
            user,
            &manager,
            &savings_vault,
            &distributor,
            &interest_vault,
        );

        self.send_and_confirm_tx(vec![instruction], None).await?;
        Ok(())
    }

    pub async fn get_account(&self, address: &Pubkey) -> Result<Account> {
        let account = self
            .ctx
            .borrow_mut()
            .banks_client
            .get_account(*address)
            .await?
            .ok_or(super::Error::AccountNotFound)?;

        Ok(account)
    }

    pub async fn get_deserialized_account<T: anchor_lang::AccountDeserialize>(
        &self,
        address: &Pubkey,
    ) -> Result<T> {
        let account = &self.get_account(address).await?;
        Ok(T::try_deserialize(&mut account.data.as_ref())?)
    }

    pub async fn send_and_confirm_tx(
        &self,
        ix: Vec<Instruction>,
        signers: Option<Vec<&Keypair>>,
    ) -> Result<()> {
        super::utils::send_and_confirm_tx(&mut self.ctx.borrow_mut(), ix, signers).await?;
        Ok(())
    }
}
