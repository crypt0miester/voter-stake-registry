use crate::{
    program_test::{
        balances, create_mint, get_account, mint_tokens, token_account_balance, AddinCookie, GovernanceCookie, GovernanceRealmCookie, RegistrarCookie, TokenOwnerRecordCookie
    },
    LifecycleTest,
};
use anchor_client::{solana_client::nonblocking::rpc_client::RpcClient, solana_sdk::signature::{Keypair, Signer}};
use anchor_lang::prelude::Pubkey;
use anchor_spl::token::TokenAccount;
use solana_program::{clock, native_token::sol_to_lamports};
use spl_associated_token_account::get_associated_token_address_with_program_id;
use voter_stake_registry::state::{LockupKind, Voter};
use std::{cell::RefCell, error::Error, sync::Arc};
use tokio::time::{sleep, Duration};
use solana_program::sysvar::Sysvar;

pub async fn delay_seconds(seconds: u64) {
    sleep(Duration::from_secs(seconds)).await;
}

pub async fn delay_ms(ms: u64) {
    sleep(Duration::from_millis(ms)).await;
}

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

    for _i in 0..num_mints {
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
            &lifecycle_test.grant_authority,
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
        AddinCookie,
        RegistrarCookie,
    ),
    Box<dyn Error>,
> {
    let governance = GovernanceCookie {
        program_id: lifecycle_test.governance_program_id.unwrap(),
    };
    let addin_program_id = lifecycle_test.addin_program_id.unwrap();
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
    delay_ms(300).await;
    let first_token_owner_record = realm
        .create_token_owner_record(
            &lifecycle_test.rpc_client,
            lifecycle_test.first_voter_authority.pubkey(),
            &lifecycle_test.first_voter_authority,
        )
        .await;

    let addin_cookie = AddinCookie {
        program_id: addin_program_id,
    };
    let registrar = addin_cookie
        .create_registrar(
            &lifecycle_test.rpc_client,
            &realm,
            &lifecycle_test.realm_authority,
            &lifecycle_test.realm_authority,
        )
        .await;
    Ok((
        governance,
        realm,
        first_token_owner_record,
        addin_cookie,
        registrar,
    ))
}

pub async fn test_basic(
    lifecycle_test: &mut LifecycleTest,
    addin_cookie: &AddinCookie,
    registrar: &RegistrarCookie,
    first_token_owner_record: &TokenOwnerRecordCookie,
) -> Result<(), Box<dyn Error>> {
    delay_seconds(1).await;
    let first_voting_mint = addin_cookie
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

    let voter = addin_cookie
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &&lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;

    // create the voter again, should have no effect
    addin_cookie
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
    delay_ms(300).await;
    let reference_initial =
        token_account_balance(&lifecycle_test.rpc_client, first_voter_first_mint_ata).await;
    let balance_initial = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(balance_initial, 0);

    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            0,
            LockupKind::Cliff,
            None,
            0,
            false,
        )
        .await?;
    delay_ms(300).await;

    addin_cookie
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
    delay_ms(300).await;

    let reference_after_deposit =
        token_account_balance(&lifecycle_test.rpc_client, first_voter_first_mint_ata).await;
    assert_eq!(reference_initial, reference_after_deposit + 10000);
    let vault_after_deposit = first_voting_mint
        .vault_balance(&lifecycle_test.rpc_client, &voter)
        .await;
    assert_eq!(vault_after_deposit, 10000);
    let balance_after_deposit = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(balance_after_deposit, 10000);

    addin_cookie
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
    delay_ms(300).await;

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
    addin_cookie
        .close_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
        )
        .await?;
    delay_ms(300).await;

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
    addin_cookie: &AddinCookie,
    registrar: &RegistrarCookie,
    first_token_owner_record: &TokenOwnerRecordCookie,
) -> Result<(), Box<dyn Error>> {
    let first_voting_mint = addin_cookie
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
    let voter = addin_cookie
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &&lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;
    delay_ms(300).await;

    // create the voter again, should have no effect
    addin_cookie
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

    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            0,
            LockupKind::Daily,
            None,
            10,
            true,
        )
        .await?;
    delay_ms(300).await;

    addin_cookie
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

    delay_ms(300).await;
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
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,
            &registrar,
            &&lifecycle_test.realm_authority,
            (3 * 24 - 1) * 60 * 60,
        )
        .await;
    delay_ms(300).await;

    addin_cookie
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
    delay_ms(300).await;

    addin_cookie
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
    delay_ms(300).await;

    addin_cookie
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
    delay_ms(300).await;

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

    addin_cookie
        .close_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
        )
        .await?;

    delay_ms(300).await;
    Ok(())
}

pub async fn test_deposit_cliff(
    lifecycle_test: &mut LifecycleTest,
    addin_cookie: &AddinCookie,
    registrar: &RegistrarCookie,
    first_token_owner_record: &TokenOwnerRecordCookie,
) -> Result<(), Box<dyn Error>> {
    let first_voting_mint = addin_cookie
        .configure_voting_mint(
            &lifecycle_test.rpc_client,
            &registrar,
            &lifecycle_test.realm_authority,
            &lifecycle_test.realm_authority,
            0,
            &lifecycle_test.first_mint_pubkey.unwrap(),
            0,
            1.0,
            1.0,
            2 * 24 * 60 * 60,
            None,
            None,
        )
        .await;
    
    let voter = addin_cookie
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;
    delay_ms(300).await;

    let voter_authority_ata = get_associated_token_address_with_program_id(
        &lifecycle_test.first_voter_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );

    let get_balances = |deposit_id| {
        balances(
            &lifecycle_test.rpc_client,
            addin_cookie,
            &registrar,
            voter_authority_ata,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            deposit_id,
        )
    };
    let withdraw = |amount: u64| {
        addin_cookie.withdraw(
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
        addin_cookie.deposit(
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

    let voter_ata_initial =
        token_account_balance(&lifecycle_test.rpc_client, voter_authority_ata).await;
    let voter_balance_initial = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(voter_balance_initial, 0);

    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            0,
            LockupKind::Cliff,
            None,
            3, // days
            false,
        )
        .await?;
    delay_ms(300).await;

    deposit(9000).await.unwrap();
    delay_ms(300).await;

    let after_deposit = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_deposit.token + after_deposit.vault);
    assert_eq!(after_deposit.voter_weight, 2 * after_deposit.vault); // saturated locking bonus
    assert_eq!(after_deposit.vault, 9000);
    assert_eq!(after_deposit.deposit, 9000);

    // cannot withdraw yet, nothing is vested
    withdraw(1).await.expect_err("nothing vested yet");

    // Advance almost 1 day
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,
            &registrar,
            &&lifecycle_test.realm_authority,
            24 * 60 * 60,
        )
        .await;
    let after_day1 = get_balances(0).await;
    delay_ms(300).await;
    assert_eq!(after_day1.voter_weight, 2 * after_day1.vault); // still saturated

    // Advance almost 1 day
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,
            &registrar,
            &&lifecycle_test.realm_authority,
            48 * 60 * 60,
        )
        .await;
    let after_day2 = get_balances(0).await;
    delay_ms(300).await;
    assert_eq!(after_day2.voter_weight, 3 * after_day2.vault / 2); // locking half done

    // Advance almost 1 day
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,
            &registrar,
            &&lifecycle_test.realm_authority,
            73 * 60 * 60,
        )
        .await;
    let after_day1 = get_balances(0).await;
    delay_ms(300).await;
    assert_eq!(after_day1.voter_weight, 2 * after_day1.vault); // still saturated

    // deposit some more
    deposit(1000).await.unwrap();
    delay_ms(300).await;

    let after_cliff = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_cliff.token + after_cliff.vault);
    assert_eq!(after_cliff.voter_weight, after_cliff.vault);
    assert_eq!(after_cliff.vault, 10000);
    assert_eq!(after_cliff.deposit, 10000);

    // can withdraw everything now
    withdraw(10001).await.expect_err("withdrew too much");
    withdraw(10000).await.unwrap();
    delay_ms(300).await;

    let after_withdraw = get_balances(0).await;
    assert_eq!(
        voter_ata_initial,
        after_withdraw.token + after_withdraw.vault
    );
    assert_eq!(after_withdraw.voter_weight, after_withdraw.vault);
    assert_eq!(after_withdraw.vault, 0);
    assert_eq!(after_withdraw.deposit, 0);

    addin_cookie
        .close_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
        )
        .await?;
    delay_ms(300).await;

    Ok(())
}

pub async fn test_deposit_constant(
    lifecycle_test: &mut LifecycleTest,
    addin_cookie: &AddinCookie,
    registrar: &RegistrarCookie,
    first_token_owner_record: &TokenOwnerRecordCookie,
) -> Result<(), Box<dyn Error>> {
    let first_voting_mint = addin_cookie
        .configure_voting_mint(
            &lifecycle_test.rpc_client,
            &registrar,
            &lifecycle_test.realm_authority,
            &lifecycle_test.realm_authority,
            0,
            &lifecycle_test.first_mint_pubkey.unwrap(),
            0,
            1.0,
            1.0,
            2 * 24 * 60 * 60,
            None,
            None,
        )
        .await;
    // not closed
    let voter = addin_cookie
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;
    delay_ms(300).await;

    let voter_authority_ata = get_associated_token_address_with_program_id(
        &lifecycle_test.first_voter_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );

    let get_balances = |deposit_id| {
        balances(
            &lifecycle_test.rpc_client,
            addin_cookie,
            &registrar,
            voter_authority_ata,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            deposit_id,
        )
    };
    let withdraw = |amount: u64| {
        addin_cookie.withdraw(
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
        addin_cookie.deposit(
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

    let voter_ata_initial =
        token_account_balance(&lifecycle_test.rpc_client, voter_authority_ata).await;
    let voter_balance_initial = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(voter_balance_initial, 0);

    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            0,
            LockupKind::Constant,
            None,
            2, // days
            false,
        )
        .await?;
    delay_ms(300).await;

    deposit(9000).await.unwrap();
    delay_ms(300).await;

    let after_deposit = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_deposit.token + after_deposit.vault);
    assert_eq!(after_deposit.voter_weight, 2 * after_deposit.vault); // saturated locking bonus
    assert_eq!(after_deposit.vault, 9000);
    assert_eq!(after_deposit.deposit, 9000);

    withdraw(1).await.expect_err("all locked up");

    // Advance almost 1 day
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,
            &registrar,
            &&lifecycle_test.realm_authority,
            3 * 24 * 60 * 60,
        )
        .await;
    delay_ms(300).await;
    let after_day3 = get_balances(0).await;
    assert_eq!(after_day3.voter_weight, after_deposit.voter_weight); // unchanged

    withdraw(1).await.expect_err("all locked up");

    deposit(1000).await.unwrap();
    delay_ms(300).await;

    let after_deposit = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_deposit.token + after_deposit.vault);
    assert_eq!(after_deposit.voter_weight, 2 * after_deposit.vault); // saturated locking bonus
    assert_eq!(after_deposit.vault, 10000);
    assert_eq!(after_deposit.deposit, 10000);

    withdraw(1).await.expect_err("all locked up");

    // Change the whole thing to cliff lockup
    addin_cookie
        .reset_lockup(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            0,
            LockupKind::Cliff,
            1,
        )
        .await
        .expect_err("can't reduce period");
    addin_cookie
        .reset_lockup(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            0,
            LockupKind::Cliff,
            2,
        )
        .await
        .unwrap();
    delay_ms(300).await;

    let after_reset = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_reset.token + after_reset.vault);
    assert_eq!(after_reset.voter_weight, 2 * after_reset.vault); // saturated locking bonus
    assert_eq!(after_reset.vault, 10000);
    assert_eq!(after_reset.deposit, 10000);

    withdraw(1).await.expect_err("all locked up");

    // advance to six days
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,
            &registrar,
            &lifecycle_test.realm_authority,
            6 * 24 * 60 * 60,
        )
        .await;
    delay_ms(300).await;

    withdraw(10000).await.unwrap();
    delay_ms(300).await;

    addin_cookie
        .close_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
        )
        .await?;
    delay_ms(300).await;

    Ok(())
}

pub async fn test_deposit_daily_vesting(
    lifecycle_test: &mut LifecycleTest,
    addin_cookie: &AddinCookie,
    registrar: &RegistrarCookie,
    first_token_owner_record: &TokenOwnerRecordCookie,
) -> Result<(), Box<dyn Error>> {
    let first_voting_mint = addin_cookie
        .configure_voting_mint(
            &lifecycle_test.rpc_client,
            &registrar,
            &lifecycle_test.realm_authority,
            &lifecycle_test.realm_authority,
            0,
            &lifecycle_test.first_mint_pubkey.unwrap(),
            0,
            1.0,
            0.5,
            60 * 60 * 60, // 60h / 2.5d
            None,
            None,
        )
        .await;
    // not closed
    let voter = addin_cookie
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;
    delay_ms(300).await;

    let voter_authority_ata = get_associated_token_address_with_program_id(
        &lifecycle_test.first_voter_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );

    let get_balances = |deposit_id: u8| {
        balances(
            &lifecycle_test.rpc_client,
            addin_cookie,
            &registrar,
            voter_authority_ata,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            deposit_id,
        )
    };
    let withdraw = |amount: u64, deposit_id: u8| {
        addin_cookie.withdraw(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            voter_authority_ata,
            deposit_id,
            amount,
        )
    };
    let deposit = |amount: u64, deposit_id: u8| {
        addin_cookie.deposit(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            voter_authority_ata,
            deposit_id,
            amount,
        )
    };

    let voter_ata_initial =
        token_account_balance(&lifecycle_test.rpc_client, voter_authority_ata).await;
    let voter_balance_initial = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(voter_balance_initial, 0);
    delay_ms(300).await;

    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            0,
            LockupKind::Daily,
            None,
            3,
            false,
        )
        .await?;
    delay_ms(300).await;
    deposit(9000, 0).await.unwrap();
    delay_ms(300).await;

    let after_deposit = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_deposit.token + after_deposit.vault);
    // The vesting parts are locked for 72, 48 and 24h. Lockup saturates at 60h.
    assert_eq!(
        after_deposit.voter_weight,
        ((after_deposit.vault as f64) * (1.0 + 0.5 * (60.0 + 48.0 + 24.0) / 60.0 / 3.0)) as u64
    );
    assert_eq!(after_deposit.vault, 9000);
    assert_eq!(after_deposit.deposit, 9000);

    // cannot withdraw yet, nothing is vested
    withdraw(1, 0).await.expect_err("nothing vested yet");

    // check vote weight reduction after an hour
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,&registrar, 
            &lifecycle_test.realm_authority, 60 * 60)
        .await;
    delay_ms(300).await;
    let after_hour = get_balances(0).await;
    assert_eq!(
        after_hour.voter_weight,
        ((after_hour.vault as f64) * (1.0 + 0.5 * (60.0 + 47.0 + 23.0) / 60.0 / 3.0)) as u64
    );

    // advance a day
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,&registrar, 
            &lifecycle_test.realm_authority, 25 * 60 * 60)
        .await;
    delay_ms(300).await;

    withdraw(3001, 0).await.expect_err("withdrew too much");
    withdraw(3000, 0).await.unwrap();
    delay_ms(300).await;

    let after_withdraw = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_withdraw.token + after_withdraw.vault);
    assert_eq!(
        after_withdraw.voter_weight,
        ((after_withdraw.vault as f64) * (1.0 + 0.5 * (47.0 + 23.0) / 60.0 / 2.0)) as u64
    );
    assert_eq!(after_withdraw.vault, 6000);
    assert_eq!(after_withdraw.deposit, 6000);

    // There are two vesting periods left, if we add 5000 to the deposit,
    // half of that should vest each day.
    deposit(5000, 0).await.unwrap();
    delay_ms(300).await;

    let after_deposit = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_deposit.token + after_deposit.vault);
    assert_eq!(
        after_deposit.voter_weight,
        ((after_deposit.vault as f64) * (1.0 + 0.5 * (47.0 + 23.0) / 60.0 / 2.0)) as u64
    );
    assert_eq!(after_deposit.vault, 11000);
    assert_eq!(after_deposit.deposit, 11000);

    withdraw(1, 0).await.expect_err("nothing vested yet");

    // advance another day
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,&registrar, 
            &lifecycle_test.realm_authority, 49 * 60 * 60)
        .await;
    delay_ms(300).await;


    // There is just one period left, should be fully withdrawable after
    deposit(1000, 0).await.unwrap();

    delay_ms(300).await;


    // can withdraw 3000 (original deposit) plus 2500 (second deposit)
    // nothing from the third deposit is vested
    withdraw(5501, 0).await.expect_err("withdrew too much");
    withdraw(5500, 0).await.unwrap();
    delay_ms(300).await;

    let after_withdraw = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_withdraw.token + after_withdraw.vault);
    assert_eq!(
        after_withdraw.voter_weight,
        ((after_withdraw.vault as f64) * (1.0 + 0.5 * 23.0 / 60.0)) as u64
    );
    assert_eq!(after_withdraw.vault, 6500);
    assert_eq!(after_withdraw.deposit, 6500);

    // advance another day
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,&registrar, 
            &lifecycle_test.realm_authority, 73 * 60 * 60)
        .await;
    delay_ms(300).await;

    // can withdraw the rest
    withdraw(6500, 0).await?;
    delay_ms(300).await;

    let after_withdraw = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_withdraw.token + after_withdraw.vault);
    assert_eq!(after_withdraw.voter_weight, after_withdraw.vault);
    assert_eq!(after_withdraw.vault, 0);
    assert_eq!(after_withdraw.deposit, 0);

    // if we deposit now, we can immediately withdraw
    deposit(1000, 0).await?;
    delay_ms(300).await;

    let after_deposit = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_deposit.token + after_deposit.vault);
    assert_eq!(after_deposit.voter_weight, after_deposit.vault);
    assert_eq!(after_deposit.vault, 1000);
    assert_eq!(after_deposit.deposit, 1000);

    withdraw(1000, 0).await?;
    delay_ms(300).await;

    let after_withdraw = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_withdraw.token + after_withdraw.vault);
    assert_eq!(after_withdraw.voter_weight, after_withdraw.vault);
    assert_eq!(after_withdraw.vault, 0);
    assert_eq!(after_withdraw.deposit, 0);

    addin_cookie
        .close_deposit_entry(
            &lifecycle_test.rpc_client,&voter, 
            &lifecycle_test.first_voter_authority, 0)
        .await?;
    delay_ms(300).await;

    //
    // Check vesting periods in the future and in the past
    //

    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,&registrar, 
            &lifecycle_test.realm_authority, 0)
        .await;
    delay_ms(300).await;

    let now = clock::Clock::get()?.unix_timestamp as u64;

    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            0,
            LockupKind::Daily,
            Some(now - 36 * 60 * 60),
            3,
            false,
        )
        .await
        .unwrap();
    delay_ms(300).await;
    deposit(30, 0).await.unwrap();
    delay_ms(300).await;

    let deposits0 = get_balances(0).await;
    // since the deposit happened late, the 30 added tokens were spread over
    // the two remaining vesting periods
    assert_eq!(
        deposits0.voter_weight,
        (30.0 + 15.0 * 0.5 * (12.0 + 36.0) / 60.0) as u64
    );
    assert_eq!(deposits0.vault, 30);
    assert_eq!(deposits0.deposit, 30);

    // the first vesting period passed without any funds in the deposit entry
    withdraw(1, 0).await.expect_err("not vested enough");

    // advance to withdraw so that we can close
    addin_cookie
    .set_time_offset(
        &lifecycle_test.rpc_client,&registrar, 
        &lifecycle_test.realm_authority, 100 * 60 * 60)
    .await;

    delay_ms(300).await;
    withdraw(30, 0).await?;
    delay_ms(300).await;


    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            1,
            LockupKind::Daily,
            Some(now + 30 * 60 * 60),
            3,
            false,
        )
        .await?;

    delay_ms(300).await;
    deposit(3000, 1).await?;
    delay_ms(300).await;

    let deposits1 = get_balances(1).await;
    assert_eq!(
        deposits1.voter_weight,
        deposits0.voter_weight + (3000.0 + 1000.0 * 0.5 * (54.0 + 60.0 + 60.0) / 60.0) as u64
    );
    assert_eq!(deposits1.vault, 3030);
    assert_eq!(deposits1.deposit, 3000);

    withdraw(1, 1).await.expect_err("not vested enough");
    // advance to withdraw so that we can close
    addin_cookie
    .set_time_offset(
        &lifecycle_test.rpc_client,&registrar, 
        &lifecycle_test.realm_authority, 100 * 60 * 60)
    .await;

    delay_ms(300).await;
    
    withdraw(3030, 1).await?;
    delay_ms(300).await;

    addin_cookie
        .close_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
        )
        .await?;
    delay_ms(300).await;

    Ok(())
}


pub async fn test_deposit_monthly_vesting(
    lifecycle_test: &mut LifecycleTest,
    addin_cookie: &AddinCookie,
    registrar: &RegistrarCookie,
    first_token_owner_record: &TokenOwnerRecordCookie,
) -> Result<(), Box<dyn Error>> {
    let first_voting_mint = addin_cookie
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
    // not closed
    let voter = addin_cookie
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;
    delay_ms(300).await;

    let voter_authority_ata = get_associated_token_address_with_program_id(
        &lifecycle_test.first_voter_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );

    let get_balances = |deposit_id| {
        balances(
            &lifecycle_test.rpc_client,
            addin_cookie,
            &registrar,
            voter_authority_ata,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            deposit_id,
        )
    };
    let withdraw = |amount: u64| {
        addin_cookie.withdraw(
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
        addin_cookie.deposit(
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

    let voter_ata_initial =
        token_account_balance(&lifecycle_test.rpc_client, voter_authority_ata).await;
    let voter_balance_initial = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(voter_balance_initial, 0);

    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            0,
            LockupKind::Monthly,
            None,
            3, // days
            false,
        )
        .await?;
    delay_ms(300).await;

    deposit(9000).await.unwrap();
    delay_ms(300).await;

    let after_deposit = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_deposit.token + after_deposit.vault);
    assert_eq!(after_deposit.voter_weight, 2 * after_deposit.vault); // saturated locking bonus
    assert_eq!(after_deposit.vault, 9000);
    assert_eq!(after_deposit.deposit, 9000);

    // cannot withdraw yet, nothing is vested
    withdraw(1).await.expect_err("nothing vested yet");

    // Advance almost 1 day
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,
            &registrar,
            &&lifecycle_test.realm_authority,
            24 * 60 * 60,
        )
        .await;
    delay_ms(300).await;
    let after_day1 = get_balances(0).await;
    assert_eq!(after_day1.voter_weight, 2 * after_day1.vault); // still saturated

    // Advance almost 1 day
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,
            &registrar,
            &&lifecycle_test.realm_authority,
            30 * 24 * 60 * 60,
        )
        .await;
    delay_ms(300).await;

    // cannot withdraw yet, nothing is vested
    withdraw(1).await.expect_err("nothing vested yet");

    // Advance almost 1 day
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,
            &registrar,
            &&lifecycle_test.realm_authority,
            32 * 24 * 60 * 60,
        )
        .await;
    delay_ms(300).await;

    withdraw(3001).await.expect_err("withdrew too much");
    withdraw(3000).await?;
    delay_ms(300).await;


    let after_withdraw = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_withdraw.token + after_withdraw.vault);
    assert_eq!(after_withdraw.voter_weight, after_withdraw.vault);
    assert_eq!(after_withdraw.vault, 6000);
    assert_eq!(after_withdraw.deposit, 6000);

    // There are two vesting periods left, if we add 5000 to the deposit,
    // half of that should vest each month.
    deposit(5000).await?;
    delay_ms(300).await;

    let after_deposit = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_deposit.token + after_deposit.vault);
    assert_eq!(after_deposit.voter_weight, after_deposit.vault);
    assert_eq!(after_deposit.vault, 11000);
    assert_eq!(after_deposit.deposit, 11000);

    withdraw(1).await.expect_err("nothing vested yet");

    // advance another month
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,
            &registrar,
            &&lifecycle_test.realm_authority,
            64 * 24 * 60 * 60)
        .await;
    delay_ms(300).await;

    // There is just one period left, should be fully withdrawable after
    deposit(1000).await?;

    delay_ms(300).await;

    // can withdraw 3000 (original deposit) plus 2500 (second deposit)
    // nothing from the third deposit is vested
    withdraw(5501).await.expect_err("withdrew too much");
    withdraw(5500).await?;

    let after_withdraw = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_withdraw.token + after_withdraw.vault);
    assert_eq!(after_withdraw.voter_weight, after_withdraw.vault);
    assert_eq!(after_withdraw.vault, 6500);
    assert_eq!(after_withdraw.deposit, 6500);

    // advance another month
    addin_cookie
        .set_time_offset(
            &lifecycle_test.rpc_client,
            &registrar,
            &lifecycle_test.realm_authority, 94 * 24 * 60 * 60)
        .await;
    delay_ms(300).await;

    // can withdraw the rest
    withdraw(6500).await.unwrap();

    let after_withdraw = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_withdraw.token + after_withdraw.vault);
    assert_eq!(after_withdraw.voter_weight, after_withdraw.vault);
    assert_eq!(after_withdraw.vault, 0);
    assert_eq!(after_withdraw.deposit, 0);

    // if we deposit now, we can immediately withdraw
    deposit(1000).await.unwrap();

    let after_deposit = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_deposit.token + after_deposit.vault);
    assert_eq!(after_deposit.voter_weight, after_deposit.vault);
    assert_eq!(after_deposit.vault, 1000);
    assert_eq!(after_deposit.deposit, 1000);

    // cannot close yet, has funds
    addin_cookie
        .close_deposit_entry(
            &lifecycle_test.rpc_client, &voter, &lifecycle_test.first_voter_authority, 0)
        .await
        .expect_err("deposit not empty");

    withdraw(1000).await?;

    let after_withdraw = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_withdraw.token + after_withdraw.vault);
    assert_eq!(after_withdraw.voter_weight, after_withdraw.vault);
    assert_eq!(after_withdraw.vault, 0);
    assert_eq!(after_withdraw.deposit, 0);

    addin_cookie
        .close_deposit_entry(&lifecycle_test.rpc_client,&voter, &lifecycle_test.first_voter_authority, 0)
        .await
        .unwrap();

    addin_cookie
        .close_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
        )
        .await?;

    delay_ms(300).await;

    Ok(())
}


pub async fn test_deposit_no_locking(
    lifecycle_test: &mut LifecycleTest,
    addin_cookie: &AddinCookie,
    registrar: &RegistrarCookie,
    first_token_owner_record: &TokenOwnerRecordCookie,
    realm:GovernanceRealmCookie
) -> Result<(), Box<dyn Error>> {
    let first_voting_mint = addin_cookie
        .configure_voting_mint(
            &lifecycle_test.rpc_client,
            &registrar,
            &lifecycle_test.realm_authority,
            &lifecycle_test.realm_authority,
            0,
            &lifecycle_test.first_mint_pubkey.unwrap(),
            0,
            1.0,
            10.0, // no locking, so has no effect
            5 * 365 * 24 * 60 * 60,
            None,
            None,
        )
        .await;
    
    let voter = addin_cookie
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;

    let second_token_owner_record = realm
        .create_token_owner_record(
            &lifecycle_test.rpc_client,
            lifecycle_test.second_voter_authority.pubkey(),
            &lifecycle_test.second_voter_authority,
        )
        .await;
    
    let voter2 = addin_cookie
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &second_token_owner_record,
            &lifecycle_test.second_voter_authority,
            &lifecycle_test.second_voter_authority,
        )
        .await;
    delay_ms(300).await;

    let voter_authority_ata = get_associated_token_address_with_program_id(
        &lifecycle_test.first_voter_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );
    let voter2_authority_ata = get_associated_token_address_with_program_id(
        &lifecycle_test.second_voter_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );

    let get_balances = |deposit_id: u8| {
        balances(
            &lifecycle_test.rpc_client,
            addin_cookie,
            &registrar,
            voter_authority_ata,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            deposit_id,
        )
    };
    let withdraw = |amount: u64, deposit_id: u8| {
        addin_cookie.withdraw(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            voter_authority_ata,
            deposit_id,
            amount,
        )
    };
    let deposit = |amount: u64, deposit_id: u8| {
        addin_cookie.deposit(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            voter_authority_ata,
            deposit_id,
            amount,
        )
    };

    let voter_ata_initial =
        token_account_balance(&lifecycle_test.rpc_client, voter_authority_ata).await;
    let voter_balance_initial = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(voter_balance_initial, 0);
    delay_ms(300).await;

    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            0,
            LockupKind::None,
            None,
            0,
            false,
        )
        .await?;
    delay_ms(300).await;
    deposit(10000, 0).await.unwrap();
    delay_ms(300).await;

    let after_deposit = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_deposit.token + after_deposit.vault);
    assert_eq!(after_deposit.voter_weight, after_deposit.vault);
    assert_eq!(after_deposit.vault, 10000);
    assert_eq!(after_deposit.deposit, 10000);

    // cannot withdraw yet, nothing is vested
    deposit(5000, 0).await.unwrap();
    delay_ms(300).await;

    let after_deposit2 = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_deposit2.token + after_deposit2.vault);
    assert_eq!(after_deposit2.voter_weight, after_deposit2.vault);
    assert_eq!(after_deposit2.vault, 15000);
    assert_eq!(after_deposit2.deposit, 15000);


    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            1,
            LockupKind::None,
            None,
            0,
            false,
        )
        .await?;

    delay_ms(300).await;

    deposit(7000, 1).await.unwrap();

    delay_ms(300).await;
    let after_deposit3 = get_balances(1).await;
    assert_eq!(voter_ata_initial, after_deposit3.token + after_deposit3.vault);
    assert_eq!(after_deposit3.voter_weight, after_deposit3.vault);
    assert_eq!(after_deposit3.vault, 22000);
    assert_eq!(after_deposit3.deposit, 7000);

    withdraw(10000, 0).await.unwrap();
    delay_ms(300).await;

    let after_withdraw1 = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_withdraw1.token + after_withdraw1.vault);
    assert_eq!(after_withdraw1.voter_weight, after_withdraw1.vault);
    assert_eq!(after_withdraw1.vault, 12000);
    assert_eq!(after_withdraw1.deposit, 5000);

    withdraw(5001, 0).await.expect_err("withdrew too much");
    withdraw(5000, 0).await.unwrap();


    let after_withdraw2 = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_withdraw2.token + after_withdraw2.vault);
    assert_eq!(after_withdraw2.voter_weight, after_withdraw2.vault);
    assert_eq!(after_withdraw2.vault, 7000);
    assert_eq!(after_withdraw2.deposit, 0);

    // Close the empty deposit (closing deposits 1 and 2 fails)

    addin_cookie
        .close_deposit_entry(
            &lifecycle_test.rpc_client,&voter, 
            &lifecycle_test.first_voter_authority, 0)
        .await
        .expect_err("deposit not in use");

        addin_cookie
        .close_deposit_entry(
            &lifecycle_test.rpc_client,&voter, 
            &lifecycle_test.first_voter_authority, 1)
        .await
        .expect_err("deposit not empty");
    addin_cookie
        .close_deposit_entry(
            &lifecycle_test.rpc_client,&voter, 
            &lifecycle_test.first_voter_authority, 0)
        .await?;
    delay_ms(300).await;

    let after_close = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_close.token + after_close.vault);
    assert_eq!(after_close.voter_weight, after_close.vault);
    assert_eq!(after_close.vault, 7000);
    assert_eq!(after_close.deposit, 0);

    delay_ms(300).await;
    let voter2_deposit = voter.deposit_amount(
        &lifecycle_test.rpc_client, 0).await;


    let voter2_voter_weight = addin_cookie
        .update_voter_weight_record(
            &lifecycle_test.rpc_client,&registrar, &voter2, &lifecycle_test.second_voter_authority)
        .await
        .unwrap()
        .voter_weight;
    assert_eq!(voter2_deposit, 0);
    assert_eq!(voter2_voter_weight, 0);
    
    // now voter2 deposits
    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter2,
            &lifecycle_test.second_voter_authority,
            &first_voting_mint,
            5,
            LockupKind::None,
            None,
            0,
            false,
        )
        .await
        .unwrap();
    addin_cookie
        .deposit(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter2,
            &first_voting_mint,
            &lifecycle_test.second_voter_authority,
            voter2_authority_ata,
            5,
            1000,
        )
        .await
        .unwrap();

    let voter2_balances = balances(
        &lifecycle_test.rpc_client,
        addin_cookie,
        &registrar,
        voter2_authority_ata,
        &voter2,
        &first_voting_mint,
        &lifecycle_test.second_voter_authority,
        5,
    )
    .await;

    assert_eq!(voter2_balances.deposit, 1000);
    assert_eq!(voter2_balances.voter_weight, 1000);
    assert_eq!(voter2_balances.vault, 1000);

    // when voter1 deposits again, they can reuse deposit index 0

    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            1,
            LockupKind::None,
            None,
            0,
            false,
        )
        .await?;
    delay_ms(300).await;

    deposit(3000, 0).await.unwrap();
    delay_ms(300).await;

    let after_reuse = get_balances(0).await;
    assert_eq!(voter_ata_initial, after_reuse.token + 7000 + 3000);
    assert_eq!(after_reuse.voter_weight, 7000 + 3000);
    assert_eq!(after_reuse.vault, 7000 + 3000);
    assert_eq!(after_reuse.deposit, 3000);
    withdraw(3000, 0).await.unwrap();
    withdraw(7000, 1).await.unwrap();
    delay_ms(300).await;

    // withdraw to close
    addin_cookie.withdraw(
        &lifecycle_test.rpc_client,
        &registrar,
        &voter2,
        &first_voting_mint,
        &lifecycle_test.second_voter_authority,
        voter2_authority_ata,
        0,
        1000,
    ).await?;
    delay_ms(300).await;


    addin_cookie
        .close_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter2,
            &first_voting_mint,
            &lifecycle_test.second_voter_authority,
        )
        .await?;

    addin_cookie
        .close_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
        )
        .await?;
    delay_ms(300).await;

    Ok(())
}


pub async fn test_grants(
    lifecycle_test: &mut LifecycleTest,
    addin_cookie: &AddinCookie,
    registrar: &RegistrarCookie,
    first_token_owner_record: &TokenOwnerRecordCookie,
) -> Result<(), Box<dyn Error>> {
    let first_voting_mint = addin_cookie
        .configure_voting_mint(
            &lifecycle_test.rpc_client,
            &registrar,
            &lifecycle_test.realm_authority,
            &lifecycle_test.realm_authority,
            0,
            &lifecycle_test.first_mint_pubkey.unwrap(),
            0,
            2.0,
            0.0,
            5 * 365 * 24 * 60 * 60,
            Some(lifecycle_test.grant_authority.pubkey()),
            None,
        )
        .await;
    
    let voter = addin_cookie
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;
    delay_ms(300).await;

    let voter_authority_ata = get_associated_token_address_with_program_id(
        &lifecycle_test.first_voter_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );
    let grant_funds = get_associated_token_address_with_program_id(
        &lifecycle_test.grant_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );
    let get_balances = |deposit_id: u8| {
        balances(
            &lifecycle_test.rpc_client,
            addin_cookie,
            &registrar,
            voter_authority_ata,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            deposit_id,
        )
    };
    let withdraw = |amount: u64, deposit_id: u8| {
        addin_cookie.withdraw(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            voter_authority_ata,
            deposit_id,
            amount,
        )
    };
    let deposit = |amount: u64, deposit_id: u8| {
        addin_cookie.deposit(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            voter_authority_ata,
            deposit_id,
            amount,
        )
    };

    let voter_ata_initial =
        token_account_balance(&lifecycle_test.rpc_client, voter_authority_ata).await;

    let grante_ata_initial =
        token_account_balance(&lifecycle_test.rpc_client, grant_funds).await;
    let voter_balance_initial = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(voter_balance_initial, 0);
    delay_ms(300).await;

    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            0,
            LockupKind::None,
            None,
            0,
            false,
        )
        .await?;
    delay_ms(300).await;
    
    // grant funds to voter (existing)
    let voter_grant = addin_cookie
        .grant(
            &lifecycle_test.rpc_client,
            &registrar,
            lifecycle_test.first_voter_authority.pubkey(),
            &first_voting_mint,
            LockupKind::Monthly,
            None,
            12,
            true,
            12000,
            grant_funds,
            &lifecycle_test.grant_authority,
            &lifecycle_test.realm_authority,
        )
        .await
        .unwrap();

    // grant funds to voter2 (new)
    let voter2_grant = addin_cookie
        .grant(
            &lifecycle_test.rpc_client,
            &registrar,
            lifecycle_test.second_voter_authority.pubkey(),
            &first_voting_mint,
            LockupKind::Monthly,
            None,
            12,
            true,
            24000,
            grant_funds,
            &lifecycle_test.grant_authority,
            &lifecycle_test.grant_authority,
        )
        .await
        .unwrap();
    delay_ms(300).await;

    assert_eq!(
        first_voting_mint
            .vault_balance(
                &lifecycle_test.rpc_client, &voter)
            .await,
        12000
    );
    assert_eq!(voter.deposit_amount(&lifecycle_test.rpc_client, 0).await, 0);
    assert_eq!(voter.deposit_amount(&lifecycle_test.rpc_client, 1).await, 12000);
    assert_eq!(voter.address, voter_grant.address);
    assert_eq!(voter2_grant.deposit_amount(&lifecycle_test.rpc_client, 0).await, 24000);

    let voter_data = get_account::<Voter>(
        &lifecycle_test.rpc_client, voter.address)
        .await;
    let deposit = &voter_data.deposits[1];
    assert_eq!(deposit.is_used, true);
    let amount_deposited_native = deposit.amount_deposited_native;
    let amount_initially_locked_native = deposit.amount_initially_locked_native;
    assert_eq!(amount_deposited_native, 12000);
    assert_eq!(amount_initially_locked_native, 12000);
    assert_eq!(deposit.allow_clawback, true);
    assert_eq!(deposit.lockup.kind, LockupKind::Monthly);
    assert_eq!(deposit.lockup.periods_total().unwrap(), 12);

    // grant funds with a start time in the past
    // by the voter authority itself
    delay_ms(300).await;
    let now = clock::Clock::get()?.unix_timestamp as u64;
    let start = now - LockupKind::Monthly.period_secs() * 2 - 60;
    addin_cookie
        .grant(
            &lifecycle_test.rpc_client,
            &registrar,
            lifecycle_test.first_voter_authority.pubkey(),
            &first_voting_mint,
            LockupKind::Monthly,
            Some(start),
            12,
            true,
            24000,
            grant_funds,
            &lifecycle_test.grant_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await
        .unwrap();

    assert_eq!(voter.deposit_amount(
        &lifecycle_test.rpc_client, 2).await, 12000);

    let voter_data = get_account::<Voter>(
        &lifecycle_test.rpc_client,voter.address)
        .await;
    let deposit = &voter_data.deposits[2];
    assert_eq!(deposit.is_used, true);
    let amount_deposited_native = deposit.amount_deposited_native;
    let amount_initially_locked_native = deposit.amount_initially_locked_native;
    assert_eq!(amount_deposited_native, 12000);
    assert_eq!(amount_initially_locked_native, 12000);
    assert_eq!(deposit.allow_clawback, true);
    assert_eq!(deposit.lockup.kind, LockupKind::Monthly);
    assert_eq!(deposit.lockup.periods_total().unwrap(), 12);
    assert_eq!(deposit.lockup.periods_left(now as i64).unwrap(), 10);
    assert_eq!(deposit.amount_unlocked(now as i64), 2000);

    // compute withdrawals
    // withdraw everything
    // close everything

    // withdraw(3000, 0).await.unwrap();
    // withdraw(7000, 1).await.unwrap();
    // delay_ms(300).await;

    // // withdraw to close
    // addin_cookie.withdraw(
    //     &lifecycle_test.rpc_client,
    //     &registrar,
    //     &voter2,
    //     &first_voting_mint,
    //     &lifecycle_test.second_voter_authority,
    //     voter2_authority_ata,
    //     0,
    //     1000,
    // ).await?;
    // delay_ms(300).await;


    // addin_cookie
    //     .close_voter(
    //         &lifecycle_test.rpc_client,
    //         &registrar,
    //         &voter2,
    //         &first_voting_mint,
    //         &lifecycle_test.second_voter_authority,
    //     )
    //     .await?;

    // addin_cookie
    //     .close_voter(
    //         &lifecycle_test.rpc_client,
    //         &registrar,
    //         &voter,
    //         &first_voting_mint,
    //         &lifecycle_test.first_voter_authority,
    //     )
    //     .await?;
    // delay_ms(300).await;

    Ok(())
}



async fn get_lockup_data(
    rpc_client: &RpcClient,
    voter: Pubkey,
    index: u8,
    time_offset: i64,
) -> (u64, u64, u64, u64, u64) {
    let now = clock::Clock::get().unwrap().unix_timestamp + time_offset;
    let voter = get_account::<voter_stake_registry::state::Voter>(rpc_client, voter)
        .await;
    let d = voter.deposits[index as usize];
    let duration = d.lockup.periods_total().unwrap() * d.lockup.kind.period_secs();
    (
        // time since lockup start (saturating at "duration")
        (duration - d.lockup.seconds_left(now)) as u64,
        // duration of lockup
        duration,
        d.amount_initially_locked_native,
        d.amount_deposited_native,
        d.amount_unlocked(now),
    )
}

pub async fn test_internal_transfer(
    lifecycle_test: &mut LifecycleTest,
    addin_cookie: &AddinCookie,
    registrar: &RegistrarCookie,
    first_token_owner_record: &TokenOwnerRecordCookie,
) -> Result<(), Box<dyn Error>> {
    let first_voting_mint = addin_cookie
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
    
    let voter = addin_cookie
        .create_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &first_token_owner_record,
            &lifecycle_test.first_voter_authority,
            &lifecycle_test.first_voter_authority,
        )
        .await;
    delay_ms(300).await;

    let voter_authority_ata = get_associated_token_address_with_program_id(
        &lifecycle_test.first_voter_authority.pubkey(),
        &lifecycle_test.first_mint_pubkey.unwrap(),
        &spl_token::id(),
    );

    let deposit = |deposit_id: u8, amount: u64| {
        addin_cookie.deposit(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
            voter_authority_ata,
            deposit_id,
            amount,
        )
    };
    let internal_transfer_locked = |source: u8, target: u8, amount: u64| {
        addin_cookie.internal_transfer_locked(
            &lifecycle_test.rpc_client,
            &registrar, &voter, 
            &lifecycle_test.first_voter_authority, source, target, amount)
    };
    let internal_transfer_unlocked = |source: u8, target: u8, amount: u64| {
        addin_cookie.internal_transfer_unlocked(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            source,
            target,
            amount,
        )
    };
    let time_offset = Arc::new(RefCell::new(0i64));
    let advance_time = |extra: u64| {
        *time_offset.borrow_mut() += extra as i64;
        addin_cookie.set_time_offset(
            &lifecycle_test.rpc_client, &registrar, 
            &lifecycle_test.realm_authority, *time_offset.borrow())
    };
    let lockup_status =
        |index: u8| get_lockup_data(
            &lifecycle_test.rpc_client, voter.address, index, *time_offset.borrow());

    let month = LockupKind::Monthly.period_secs();
    let day = 24 * 60 * 60;
    let hour = 60 * 60;


    let voter_ata_initial =
        token_account_balance(&lifecycle_test.rpc_client, voter_authority_ata).await;
    let voter_balance_initial = voter.deposit_amount(&lifecycle_test.rpc_client, 0).await;
    assert_eq!(voter_balance_initial, 0);
    delay_ms(300).await;

    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            0,
            LockupKind::Monthly,
            None,
            3,
            false,
        )
        .await?;
    delay_ms(300).await;
    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            1,
            LockupKind::Daily,
            None,
            3,
            false,
        )
        .await?;
    delay_ms(300).await;
    deposit(0, 300).await.unwrap();
    deposit(1, 30).await.unwrap();
    delay_ms(300).await;

    // both deposits have vested one period
    advance_time(day + hour).await;
    assert_eq!(
        lockup_status(0).await,
        (month + day + 2 * hour, 3 * month, 300, 300, 100)
    );
    assert_eq!(lockup_status(1).await, (day + hour, 3 * day, 30, 30, 10));

    internal_transfer_locked(0, 1, 1)
        .await
        .expect_err("can't make less strict/period");
    internal_transfer_locked(1, 0, 21)
        .await
        .expect_err("can only transfer locked");
    internal_transfer_locked(1, 0, 10).await.unwrap();
    delay_ms(300).await;
    assert_eq!(
        lockup_status(0).await,
        (day + 2 * hour, 2 * month, 210, 310, 100)
    );
    assert_eq!(lockup_status(1).await, (hour, 2 * day, 10, 20, 10));

    //
    // test transfering unlocked funds
    //

    internal_transfer_unlocked(2, 0, 1000)
        .await
        .expect_err("deposit entry not in use");
    internal_transfer_unlocked(1, 0, 11)
        .await
        .expect_err("amount too high");
    internal_transfer_unlocked(1, 0, 10).await.unwrap();
    assert_eq!(
        lockup_status(0).await,
        (day + 2 * hour, 2 * month, 210, 320, 110)
    );
    assert_eq!(lockup_status(1).await, (hour, 2 * day, 10, 10, 0));

    internal_transfer_unlocked(0, 1, 100).await.unwrap();
    assert_eq!(
        lockup_status(0).await,
        (day + 2 * hour, 2 * month, 210, 220, 10)
    );
    assert_eq!(lockup_status(1).await, (hour, 2 * day, 10, 110, 100));
    
    //
    // test partially moving tokens from constant deposit to cliff
    //

    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            2,
            LockupKind::Constant,
            None,
            5,
            false,
        )
        .await?;
    delay_ms(300).await;
    addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            3,
            LockupKind::Cliff,
            None,
            5,
            false,
        )
        .await?;
    delay_ms(300).await;
    assert_eq!(lockup_status(2).await, (0, 5 * day, 1000, 1000, 0));
    assert_eq!(lockup_status(3).await, (0, 5 * day, 0, 0, 0));

    internal_transfer_locked(2, 3, 100).await.unwrap();
    delay_ms(300).await;
    assert_eq!(lockup_status(2).await, (0, 5 * day, 900, 900, 0));
    assert_eq!(lockup_status(3).await, (0, 5 * day, 100, 100, 0));

    advance_time(2 * day + hour).await;

    internal_transfer_locked(2, 3, 100)
        .await
        .expect_err("target deposit has not enough period left");

        addin_cookie
        .create_deposit_entry(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &lifecycle_test.first_voter_authority,
            &first_voting_mint,
            4,
            LockupKind::Cliff,
            None,
            8,
            false,
        )
        .await?;
    delay_ms(300).await;
    internal_transfer_locked(2, 4, 100).await.unwrap();

    assert_eq!(lockup_status(2).await, (0, 5 * day, 800, 800, 0));
    assert_eq!(
        lockup_status(3).await,
        (2 * day + hour, 5 * day, 100, 100, 0)
    );
    assert_eq!(lockup_status(4).await, (0, 8 * day, 100, 100, 0));

    advance_time(day + hour).await;
    delay_ms(300).await;

    // still ok, cliff deposit 4 still has 7 days of lockup left, which is >= 5
    internal_transfer_locked(2, 4, 800).await.unwrap();

    assert_eq!(lockup_status(2).await, (0, 5 * day, 0, 0, 0));
    assert_eq!(lockup_status(4).await, (hour, 7 * day, 900, 900, 0));
    
    addin_cookie
        .close_voter(
            &lifecycle_test.rpc_client,
            &registrar,
            &voter,
            &first_voting_mint,
            &lifecycle_test.first_voter_authority,
        )
        .await?;
    delay_ms(300).await;

    Ok(())
}
