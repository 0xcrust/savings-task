use anyhow::Result;
use clap::Parser;

use anchor_lang::{AccountDeserialize, Discriminator, InstructionData, ToAccountMetas};
use anchor_spl::associated_token::get_associated_token_address;

use solana_account_decoder::UiAccountEncoding;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::{
    RpcAccountInfoConfig, RpcProgramAccountsConfig, RpcSendTransactionConfig,
};
use solana_client::rpc_filter::{Memcmp, RpcFilterType};
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use solana_sdk::signer::EncodableKey;
use solana_sdk::transaction::Transaction;

use savings_program::SavingsManager;
use savings_program::{accounts, instruction};

#[derive(Debug, Parser)]
pub struct Cli {
    #[clap(default_value = "mainnet")]
    pub cluster_or_url: String,
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Parser)]
pub enum Command {
    Crank {
        #[clap(long, short)]
        keypair: String,

        #[clap(long, short)]
        user_pubkey: Pubkey,

        #[clap(long, short)]
        program_id: Pubkey,
    },
}

fn cluster_url(cluster: &str) -> &str {
    match cluster {
        "devnet" => "https://api.devnet.solana.com",
        "testnet" => "https://api.testnet.solana.com",
        "mainnet" => "https://api.mainnet-beta.solana.com",
        "localnet" => "http://127.0.0.1:8899",
        custom => custom,
    }
}

impl Cli {
    fn rpc_client(&self) -> RpcClient {
        RpcClient::new(cluster_url(&self.cluster_or_url).to_string())
    }
}

/// Fetches all the savings accounts for a particular user and runs the crank on them.
async fn get_user_accounts(
    user: &Pubkey,
    rpc: &RpcClient,
    program: &Pubkey,
) -> Result<Vec<(Pubkey, SavingsManager)>> {
    let account_type_filter = RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
        0,
        SavingsManager::DISCRIMINATOR.to_vec(),
    ));
    let user_filter = RpcFilterType::Memcmp(Memcmp::new_raw_bytes(8, user.to_bytes().to_vec()));

    let config = RpcProgramAccountsConfig {
        filters: Some(vec![account_type_filter, user_filter]),
        account_config: RpcAccountInfoConfig {
            encoding: Some(UiAccountEncoding::Base64),
            commitment: Some(rpc.commitment()),
            ..RpcAccountInfoConfig::default()
        },
        with_context: Some(true),
    };

    let accounts = rpc
        .get_program_accounts_with_config(program, config)
        .await?;
    Ok(accounts
        .iter()
        .map(|(key, account)| {
            let manager = SavingsManager::try_deserialize(&mut account.data.as_ref()).unwrap();
            (*key, manager)
        })
        .collect())
}

async fn crank(
    keypair_path: String,
    user_pubkey: Pubkey,
    client: &RpcClient,
    program: &Pubkey,
) -> Result<()> {
    let payer = solana_sdk::signature::Keypair::read_from_file(keypair_path)
        .map_err(|_| anyhow::anyhow!("failed reading keypair from path"))?;
    let accounts = get_user_accounts(&user_pubkey, client, program).await?;

    let mut instructions = Vec::with_capacity(accounts.len());
    for (manager, manager_account) in accounts {
        let data = instruction::DepositInterest {}.data();
        let accounts = accounts::DepositInterestToUser {
            user: user_pubkey,
            user_savings_manager: manager,
            user_savings_vault: get_associated_token_address(&manager, &manager_account.mint),
            interest_distributor: manager_account.distributor,
            interest_vault: get_associated_token_address(
                &manager_account.distributor,
                &manager_account.mint,
            ),
            token_program: anchor_spl::token::ID,
        };
        let instruction = Instruction {
            program_id: *program,
            accounts: accounts.to_account_metas(None),
            data,
        };
        instructions.push(instruction);
    }

    for instruction in instructions {
        let recent_hash = client.get_latest_blockhash().await?;
        let signers = vec![&payer];
        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &signers,
            recent_hash,
        );

        client
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx,
                CommitmentConfig::confirmed(),
                RpcSendTransactionConfig {
                    skip_preflight: true,
                    ..RpcSendTransactionConfig::default()
                },
            )
            .await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = cli.rpc_client();
    match cli.command {
        Command::Crank {
            keypair,
            user_pubkey,
            program_id,
        } => crank(keypair, user_pubkey, &client, &program_id).await?,
    }

    Ok(())
}
