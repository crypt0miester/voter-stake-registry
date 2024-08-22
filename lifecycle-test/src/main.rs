use anchor_client::solana_client::nonblocking::rpc_client::RpcClient;
use anchor_client::solana_sdk::native_token::sol_to_lamports;
use anchor_client::solana_sdk::signature::read_keypair_file;
use anchor_spl::token::TokenAccount;
use program_test::{create_mint, governance, mint_tokens, token_account_balance, GovernanceCookie, VsrCookie};
use spl_associated_token_account::get_associated_token_address_with_program_id;
use voter_stake_registry::state::Voter;
use vsr_lifecycle::{fund_keypairs, initialize_realm_accounts, setup_mints_and_tokens, test_basic, test_clawback};
use std::process::Command;

use anchor_client::solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use std::str::FromStr;
use std::{error::Error, sync::Arc};
mod program_test;
mod vsr_lifecycle;

pub struct LifecycleTest {
    pub rpc_client: Arc<RpcClient>,
    pub realm_authority: Keypair,
    pub first_voter_authority: Keypair,
    pub second_voter_authority: Keypair,
    pub community_mint_pubkey: Option<Pubkey>,
    pub first_mint_pubkey: Option<Pubkey>,
    pub second_mint_pubkey: Option<Pubkey>,
    pub program_id: Option<Pubkey>,
}

async fn deploy_program(program_path: &str, rpc_url: &str) -> Result<Pubkey, Box<dyn Error>> {
    // Generate a new keypair for the program
    let output = Command::new("solana")
        .args(&[
            "program",
            "deploy",
            "--keypair",
            "program-auth-keypair.json",
            "--keypair",
            "program-auth-keypair.json",
            "-u",
            &rpc_url,
            program_path,
        ])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Failed to deploy program: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    // Extract the program ID from the output
    let output_str = String::from_utf8_lossy(&output.stdout);
    let program_id_str = output_str
        .lines()
        .find(|line| line.starts_with("Program Id: "))
        .and_then(|line| line.strip_prefix("Program Id: "))
        .ok_or("Failed to find Program Id in output")?;

    // Parse the program ID string into a Pubkey
    let program_id = Pubkey::from_str(program_id_str)?;

    println!("Deployed program with ID: {}", program_id);

    Ok(program_id)
}

async fn upgrade_program(
    program_id: &Pubkey,
    rpc_url: &str,
    upgrade_path: &str,
) -> Result<(), Box<dyn Error>> {
    // Convert the Pubkey to a string
    let program_id_str = program_id.to_string();

    // Run the Solana CLI command to upgrade the program
    let output = Command::new("solana")
        .args(&[
            "program",
            "deploy",
            "--keypair",
            "program-auth-keypair.json",
            "-u",
            &rpc_url,
            "--program-id",
            &program_id_str,
            upgrade_path,
        ])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Failed to upgrade program: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    println!("Successfully upgraded program: {}", program_id);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // initialize localnet validator
    let rpc_url = "http://localhost:8899";
    let rpc_client = Arc::new(RpcClient::new(rpc_url.to_string()));

    // initialize keypairs
    let realm_authority_keypair = Keypair::new();
    let first_voter_authority_keypair = Keypair::new();
    let second_voter_authority_keypair = Keypair::new();
    let mut lifecycle_test = LifecycleTest {
        rpc_client: rpc_client.clone(),
        realm_authority: realm_authority_keypair,
        first_voter_authority: first_voter_authority_keypair,
        second_voter_authority: second_voter_authority_keypair,
        community_mint_pubkey: None,
        first_mint_pubkey: None,
        second_mint_pubkey: None,
        program_id: None,
    };
    let program_authority_keypair = read_keypair_file("lifecycle-test/keypairs/auth-keypair.json")
        .expect("Failed to read keypair file");
    let program_keypair = read_keypair_file("lifecycle-test/keypairs/program-keypair.json")
        .expect("Failed to read keypair file");

    lifecycle_test.program_id = Some(program_keypair.pubkey());


    // let balance = rpc_client.get_balance(&program_authority_keypair.pubkey()).await?;
    // println!("balance: {}", balance);

    lifecycle_test.rpc_client
        .request_airdrop(&program_authority_keypair.pubkey(), sol_to_lamports(100.0))
        .await?;
    // fund
    fund_keypairs(&mut lifecycle_test).await?;
    
    // initialize mints and token accounts
    setup_mints_and_tokens(&mut lifecycle_test, 3).await?;

    let (governance, realm, first_token_owner_record, vsr_addin, registrar, first_voting_mint) = initialize_realm_accounts(&mut lifecycle_test).await?;
    
    test_basic(&mut lifecycle_test, &vsr_addin, &registrar, &first_token_owner_record, &first_voting_mint).await?;
    test_clawback(&mut lifecycle_test, &vsr_addin, &registrar, &first_token_owner_record, &first_voting_mint).await?;
    
    
    println!("Upgraded to spl_governance_4.so");

    Ok(())
}
