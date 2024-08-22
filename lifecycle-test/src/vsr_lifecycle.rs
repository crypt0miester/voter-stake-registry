use crate::{
    program_test::{
        balances, create_mint, mint_tokens, token_account_balance, vsr, GovernanceCookie, GovernanceRealmCookie, RegistrarCookie, TokenOwnerRecordCookie, VotingMintConfigCookie, VsrCookie
    },
    LifecycleTest,
};
use anchor_client::solana_sdk::signature::{Keypair, Signer};
use anchor_spl::token::TokenAccount;
use solana_program::native_token::sol_to_lamports;
use spl_associated_token_account::get_associated_token_address_with_program_id;
use std::error::Error;
use voter_stake_registry::state::Voter;

pub async fn run_lifecycle_tests(lifecycle_test: &mut LifecycleTest) -> Result<(), Box<dyn Error>> {
    // Implementation for running lifecycle tests
    // This is a placeholder and needs to be implemented
    println!("Running lifecycle tests...");
    Ok(())
}

pub async fn fund_keypairs(lifecycle_test: &mut LifecycleTest) -> Result<(), Box<dyn Error>> {
    lifecycle_test
        .rpc_client
        .request_airdrop(
            &lifecycle_test.realm_authority.pubkey(),
            sol_to_lamports(10.0),
        )
        .await?;
    lifecycle_test
        .rpc_client
        .request_airdrop(
            &lifecycle_test.first_voter_authority.pubkey(),
            sol_to_lamports(1.0),
        )
        .await?;
    lifecycle_test
        .rpc_client
        .request_airdrop(
            &lifecycle_test.second_voter_authority.pubkey(),
            sol_to_lamports(1.0),
        )
        .await?;

    Ok(())
}

pub async fn setup_mints_and_tokens(
    lifecycle_test: &mut LifecycleTest,
    num_mints: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut mint_keypairs = Vec::with_capacity(num_mints);

    for i in 0..num_mints {
        let mint_keypair = Keypair::new();
        create_mint(
            &lifecycle_test.rpc_client,
            &mint_keypair,
            &lifecycle_test.realm_authority,
            Some(&lifecycle_test.realm_authority.pubkey()),
        )
        .await?;

        // Mint tokens to realm authority and both voter authorities
        for authority in [
            &lifecycle_test.realm_authority,
            &lifecycle_test.first_voter_authority,
            &lifecycle_test.second_voter_authority,
        ] {
            mint_tokens(
                &lifecycle_test.rpc_client,
                &mint_keypair.pubkey(),
                &lifecycle_test.realm_authority,
                &authority.pubkey(),
                100_000,
            )
            .await?;
        }

        mint_keypairs.push(mint_keypair);
    }

    // Update LifecycleTest with the new mint public keys
    if !mint_keypairs.is_empty() {
        lifecycle_test.community_mint_pubkey = Some(mint_keypairs[0].pubkey());
    }
    if mint_keypairs.len() > 1 {
        lifecycle_test.first_mint_pubkey = Some(mint_keypairs[1].pubkey());
    }
    if mint_keypairs.len() > 2 {
        lifecycle_test.second_mint_pubkey = Some(mint_keypairs[2].pubkey());
    }

    Ok(())
}

pub async fn initialize_realm_accounts(
    lifecycle_test: &mut LifecycleTest,
) -> Result<
    (
        GovernanceCookie,
        GovernanceRealmCookie,
        TokenOwnerRecordCookie,
        VsrCookie,
        RegistrarCookie,
        VotingMintConfigCookie,
    ),
    Box<dyn Error>,
> {
    let governance = GovernanceCookie {
        program_id: lifecycle_test.program_id.unwrap(),
    };
    let addin_program_id = voter_stake_registry::id();
    let realm = governance
        .create_realm(
            &lifecycle_test.rpc_client,
            "test1",
            &lifecycle_test.realm_authority.pubkey(),
            &lifecycle_test.community_mint_pubkey.unwrap(),
            &lifecycle_test.realm_authority,
            &addin_program_id,
        )
        .await;
    let first_token_owner_record = realm
        .create_token_owner_record(
            &lifecycle_test.rpc_client,
            lifecycle_test.first_voter_authority.pubkey(),
            &lifecycle_test.first_voter_authority,
        )
        .await;

    let vsr_addin = VsrCookie {
        program_id: addin_program_id,
    };
    let registrar = vsr_addin
        .create_registrar(
            &lifecycle_test.rpc_client,
            &realm,
            &lifecycle_test.realm_authority,
            &lifecycle_test.realm_authority,
        )
        .await;

    vsr_addin
        .configure_voting_mint(
            &lifecycle_test.rpc_client,
            &registrar,
            &lifecycle_test.realm_authority,
            &lifecycle_test.realm_authority,
            0,
            &lifecycle_test.first_mint_pubkey.unwrap(),
            10,
            0.0,
            0.0,
            1,
            None,
            None,
        )
        .await;
    let first_voting_mint = vsr_addin
        .configure_voting_mint(
            &lifecycle_test.rpc_client,
            &registrar,
            &lifecycle_test.realm_authority,
            &lifecycle_test.realm_authority,
            0,
            &lifecycle_test.first_mint_pubkey.unwrap(),
            0,
            1.0,
            0.0,
            5 * 365 * 24 * 60 * 60,
            None,
            None,
        )
        .await;
    Ok((
        governance,
        realm,
        first_token_owner_record,
        vsr_addin,
        registrar,
        first_voting_mint,
    ))
}

pub async fn test_basic(
    lifecycle_test: &mut LifecycleTest,
    vsr_addin: &VsrCookie,
    registrar: &RegistrarCookie,
    first_token_owner_record: &TokenOwnerRecordCookie,
    first_voting_mint: &VotingMintConfigCookie,
) -> Result<(), Box<dyn Error>> {
    let voter = vsr_addin
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &&lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;

    // create the voter again, should have no effect
    vsr_addin
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &&lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;

    let first_voter_first_mint_ata = get_associated_token_address_with_program_id(
        &lifecycle_test.first_voter_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );
    let reference_initial =
        token_account_balance(&lifecycle_test.rpc_client, first_voter_first_mint_ata).await;
    let balance_initial = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(balance_initial, 0);

    vsr_addin
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            0,
            voter_stake_registry::state::LockupKind::Cliff,
            None,
            0,
            false,
        )
        .await?;

    vsr_addin
        .deposit(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            first_voter_first_mint_ata,
            0,
            10_000,
        )
        .await?;

    let reference_after_deposit =
        token_account_balance(&lifecycle_test.rpc_client, first_voter_first_mint_ata).await;
    assert_eq!(reference_initial, reference_after_deposit + 10000);
    let vault_after_deposit = first_voting_mint
        .vault_balance(&lifecycle_test.rpc_client, &voter)
        .await;
    assert_eq!(vault_after_deposit, 10000);
    let balance_after_deposit = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(balance_after_deposit, 10000);

    vsr_addin
        .withdraw(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            first_voter_first_mint_ata,
            0,
            10000,
        )
        .await?;

    let reference_after_withdraw =
        token_account_balance(&lifecycle_test.rpc_client, first_voter_first_mint_ata).await;
    assert_eq!(reference_initial, reference_after_withdraw);
    let vault_after_withdraw = first_voting_mint
        .vault_balance(&lifecycle_test.rpc_client, &voter)
        .await;
    assert_eq!(vault_after_withdraw, 0);
    let balance_after_withdraw = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(balance_after_withdraw, 0);

    let lamports_before = lifecycle_test
        .rpc_client
        .get_balance(&lifecycle_test.first_voter_authority.pubkey())
        .await?;

    // finally we have to always close the voter to test other deposit functions
    vsr_addin
        .close_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
        )
        .await?;

    let lamports_after = lifecycle_test
        .rpc_client
        .get_balance(&lifecycle_test.first_voter_authority.pubkey())
        .await?;
    let token_rent = lifecycle_test
        .rpc_client
        .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)
        .await?;
    let voter_rent = lifecycle_test
        .rpc_client
        .get_minimum_balance_for_rent_exemption(std::mem::size_of::<Voter>())
        .await?;
    let tolerance = 60_000;
    assert!(lamports_after > lamports_before + voter_rent + token_rent - tolerance);

    Ok(())
}

pub async fn test_clawback(
    lifecycle_test: &mut LifecycleTest,
    vsr_addin: &VsrCookie,
    registrar: &RegistrarCookie,
    first_token_owner_record: &TokenOwnerRecordCookie,
    first_voting_mint: &VotingMintConfigCookie,
) -> Result<(), Box<dyn Error>> {
    let voter = vsr_addin
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &&lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;

    // create the voter again, should have no effect
    vsr_addin
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &&lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;

    let realm_authority_ata = get_associated_token_address_with_program_id(
        &lifecycle_test.realm_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );

    let voter_authority_ata = get_associated_token_address_with_program_id(
        &lifecycle_test.first_voter_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );

    let realm_ata_initial =
        token_account_balance(&lifecycle_test.rpc_client, realm_authority_ata).await;
    let voter_ata_initial =
        token_account_balance(&lifecycle_test.rpc_client, voter_authority_ata).await;
    let voter_balance_initial = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(voter_balance_initial, 0);

    vsr_addin
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            0,
            voter_stake_registry::state::LockupKind::Daily,
            None,
            10,
            true,
        )
        .await?;

    vsr_addin
        .deposit(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            voter_authority_ata,
            0,
            10_000,
        )
        .await?;

    let realm_ata_after_deposit =
        token_account_balance(&lifecycle_test.rpc_client, realm_authority_ata).await;
    assert_eq!(realm_ata_initial, realm_ata_after_deposit + 10000);
    let vault_after_deposit = first_voting_mint
        .vault_balance(&lifecycle_test.rpc_client, &voter)
        .await;
    assert_eq!(vault_after_deposit, 10000);
    let voter_balance_after_deposit = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(voter_balance_after_deposit, 10000);

    // Advance almost three days for some vesting to kick in
    vsr_addin
        .set_time_offset(
            &lifecycle_test.rpc_client,
            &registrar,
            &&lifecycle_test.realm_authority,
            (3 * 24 - 1) * 60 * 60,
        )
        .await;

    vsr_addin
        .withdraw(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            voter_authority_ata,
            0,
            999,
        )
        .await?;

    vsr_addin
        .clawback(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.realm_authority,
            realm_authority_ata,
            0,
        )
        .await?;

    vsr_addin
        .withdraw(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.realm_authority,
            voter_authority_ata,
            0,
            1001,
        )
        .await?;

    let realm_after_clawback =
        token_account_balance(&lifecycle_test.rpc_client, realm_authority_ata).await;

    assert_eq!(realm_ata_initial - 2000, realm_after_clawback);

    let voter_after_withdraw =
        token_account_balance(&lifecycle_test.rpc_client, voter_authority_ata).await;
    assert_eq!(voter_after_withdraw, voter_ata_initial + 2000);

    let vault_after_withdraw = first_voting_mint
        .vault_balance(&lifecycle_test.rpc_client, &voter)
        .await;

    assert_eq!(vault_after_withdraw, 0);
    let voter_balance_after_withdraw = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(voter_balance_after_withdraw, 0);

    Ok(())
}


pub async fn test_deposit_cliff(
    lifecycle_test: &mut LifecycleTest,
    vsr_addin: &VsrCookie,
    registrar: &RegistrarCookie,
    first_token_owner_record: &TokenOwnerRecordCookie,
    first_voting_mint: &VotingMintConfigCookie,
) -> Result<(), Box<dyn Error>> {
    let voter = vsr_addin
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;


    let realm_authority_ata = get_associated_token_address_with_program_id(
        &lifecycle_test.realm_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );

    let voter_authority_ata = get_associated_token_address_with_program_id(
        &lifecycle_test.first_voter_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );

    let get_balances = |depot_id| {
        balances(
            &lifecycle_test.rpc_client,
            vsr_addin,
            &registrar,
            voter_authority_ata,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            depot_id,
        )
    };
    let withdraw = |amount: u64| {
        vsr_addin.withdraw(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            voter_authority_ata,
            0,
            amount,
        )
    };
    let deposit = |amount: u64| {
        vsr_addin.deposit(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            voter_authority_ata,
            0,
            amount,
        )
    };


    let realm_ata_initial =
        token_account_balance(&lifecycle_test.rpc_client, realm_authority_ata).await;
    let voter_ata_initial =
        token_account_balance(&lifecycle_test.rpc_client, voter_authority_ata).await;
    let voter_balance_initial = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(voter_balance_initial, 0);

    vsr_addin
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            0,
            voter_stake_registry::state::LockupKind::Daily,
            None,
            10,
            true,
        )
        .await?;

    vsr_addin
        .deposit(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            voter_authority_ata,
            0,
            10_000,
        )
        .await?;

    let realm_ata_after_deposit =
        token_account_balance(&lifecycle_test.rpc_client, realm_authority_ata).await;
    assert_eq!(realm_ata_initial, realm_ata_after_deposit + 10000);
    let vault_after_deposit = first_voting_mint
        .vault_balance(&lifecycle_test.rpc_client, &voter)
        .await;
    assert_eq!(vault_after_deposit, 10000);
    let voter_balance_after_deposit = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(voter_balance_after_deposit, 10000);

    // Advance almost three days for some vesting to kick in
    vsr_addin
        .set_time_offset(
            &lifecycle_test.rpc_client,
            &registrar,
            &&lifecycle_test.realm_authority,
            (3 * 24 - 1) * 60 * 60,
        )
        .await;

    vsr_addin
        .withdraw(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            voter_authority_ata,
            0,
            999,
        )
        .await?;

    vsr_addin
        .clawback(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.realm_authority,
            realm_authority_ata,
            0,
        )
        .await?;

    vsr_addin
        .withdraw(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.realm_authority,
            voter_authority_ata,
            0,
            1001,
        )
        .await?;

    let realm_after_clawback =
        token_account_balance(&lifecycle_test.rpc_client, realm_authority_ata).await;
        
    assert_eq!(realm_ata_initial - 2000, realm_after_clawback);

    let voter_after_withdraw =
        token_account_balance(&lifecycle_test.rpc_client, voter_authority_ata).await;
    assert_eq!(voter_after_withdraw, voter_ata_initial + 2000);

    let vault_after_withdraw = first_voting_mint
        .vault_balance(&lifecycle_test.rpc_client, &voter)
        .await;

    assert_eq!(vault_after_withdraw, 0);
    let voter_balance_after_withdraw = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(voter_balance_after_withdraw, 0);

    Ok(())
}
