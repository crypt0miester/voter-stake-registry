use addin_lifecycle::{
    fund_keypairs, initialize_realm_accounts, setup_mints_and_tokens, test_basic, test_clawback,
    test_deposit_cliff, test_deposit_constant, test_deposit_daily_vesting, test_deposit_monthly_vesting, test_deposit_no_locking, test_grants,
};
use anchor_client::solana_client::nonblocking::rpc_client::RpcClient;
use anchor_client::solana_sdk::commitment_config::CommitmentConfig;
use anchor_client::solana_sdk::signature::read_keypair_file;

use anchor_client::solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use std::{error::Error, sync::Arc};
mod addin_lifecycle;
mod program_deploy;
mod program_test;

pub struct LifecycleTest {
    pub rpc_client: Arc<RpcClient>,
    pub realm_authority: Keypair,
    pub first_voter_authority: Keypair,
    pub second_voter_authority: Keypair,
    pub grant_authority: Keypair,
    pub community_mint_pubkey: Option<Pubkey>,
    pub first_mint_pubkey: Option<Pubkey>,
    pub second_mint_pubkey: Option<Pubkey>,
    pub governance_program_id: Option<Pubkey>,
    pub addin_program_id: Option<Pubkey>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let rpc_url = "http://localhost:8899";
    let rpc_client = Arc::new(RpcClient::new_with_commitment(
        rpc_url.to_string(),
        CommitmentConfig::confirmed(),
    ));

    // initialize keypairs
    let realm_authority_keypair = Keypair::new();
    let first_voter_authority_keypair = Keypair::new();
    let second_voter_authority_keypair = Keypair::new();
    let grant_authority_keypair = Keypair::new();
    let mut lifecycle_test = LifecycleTest {
        rpc_client: rpc_client.clone(),
        realm_authority: realm_authority_keypair,
        first_voter_authority: first_voter_authority_keypair,
        second_voter_authority: second_voter_authority_keypair,
        grant_authority: grant_authority_keypair,
        community_mint_pubkey: None,
        first_mint_pubkey: None,
        second_mint_pubkey: None,
        governance_program_id: None,
        addin_program_id: None,
    };

    let goverenance_program_pubkey =
        read_keypair_file("lifecycle-test/keypairs/goverenance-program-keypair.json")
            .expect("Failed to read keypair file")
            .pubkey();
    let vsr_program_pubkey = read_keypair_file("lifecycle-test/keypairs/vsr-program-keypair.json")
        .expect("Failed to read keypair file")
        .pubkey();

    lifecycle_test.governance_program_id = Some(goverenance_program_pubkey);
    lifecycle_test.addin_program_id = Some(vsr_program_pubkey);

    // fund
    fund_keypairs(&mut lifecycle_test).await?;

    // initialize mints and token accounts
    setup_mints_and_tokens(&mut lifecycle_test, 3).await?;

    let (governance, realm, first_token_owner_record, addin_cookie, registrar) =
        initialize_realm_accounts(&mut lifecycle_test).await?;

    test_basic(
        &mut lifecycle_test,
        &addin_cookie,
        &registrar,
        &first_token_owner_record,
    )
    .await?;

    test_clawback(
        &mut lifecycle_test,
        &addin_cookie,
        &registrar,
        &first_token_owner_record,
    )
    .await?;

    test_deposit_cliff(
        &mut lifecycle_test,
        &addin_cookie,
        &registrar,
        &first_token_owner_record,
    )
    .await?;

    test_deposit_constant(
        &mut lifecycle_test,
        &addin_cookie,
        &registrar,
        &first_token_owner_record,
    )
    .await?;

    test_deposit_daily_vesting(
        &mut lifecycle_test,
        &addin_cookie,
        &registrar,
        &first_token_owner_record,
    )
    .await?;

    test_deposit_monthly_vesting(&mut lifecycle_test, &addin_cookie, &registrar, &first_token_owner_record).await?;
    test_deposit_no_locking(&mut lifecycle_test, &addin_cookie, &registrar, &first_token_owner_record, realm).await?;
    
    // requires some withdrawals and closing of the voter account
    // test_grants(&mut lifecycle_test, &addin_cookie, &registrar, &first_token_owner_record).await?;

    println!("Upgraded to spl_governance_4.so");

    Ok(())
}
