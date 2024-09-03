use std::error::Error;

use anchor_client::solana_client::nonblocking::rpc_client::RpcClient;
use anchor_client::solana_sdk::signature::{Keypair, Signature};
use anchor_client::solana_sdk::signer::Signer;
use anchor_client::solana_sdk::transaction::Transaction;
use anchor_lang::prelude::Pubkey;
use anchor_lang::AccountDeserialize;
use anchor_spl::token::TokenAccount;
use solana_program::instruction::Instruction;
use solana_program::{program_pack::Pack, system_instruction};
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token::*;

#[allow(dead_code)]
pub async fn get_transaction_logs(
    rpc_client: &RpcClient,
    signature: &Signature,
) -> Result<Vec<String>, Box<dyn Error>> {
    // Fetch the transaction
    let transaction = rpc_client
        .get_transaction(
            &signature,
            solana_transaction_status::UiTransactionEncoding::JsonParsed,
        )
        .await?;

    // Extract and return the logs
    Ok(transaction
        .transaction
        .meta
        .and_then(|meta| meta.log_messages.into())
        .unwrap_or_default())
}

#[allow(dead_code)]
pub async fn process_transaction(
    rpc_client: &RpcClient,
    instructions: &[Instruction],
    payer: &Keypair,
    signers: Option<&[&Keypair]>,
) -> Result<Signature, Box<dyn Error>> {
    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));

    let mut all_signers = vec![payer];

    if let Some(signers) = signers {
        all_signers.extend_from_slice(signers);
    }

    let lastest_blockhash = rpc_client.get_latest_blockhash().await.unwrap();

    transaction.sign(&all_signers, lastest_blockhash);

    let signature = rpc_client
        .send_and_confirm_transaction(&transaction)
        .await?;

    return Ok(signature);
}

#[allow(dead_code)]
pub async fn create_token_account(
    rpc_client: &RpcClient,
    payer: &Keypair,
    owner: &Pubkey,
    mint: Pubkey,
) -> Pubkey {
    let keypair = Keypair::new();
    let rent = rpc_client
        .get_minimum_balance_for_rent_exemption(spl_token::state::Account::LEN)
        .await
        .unwrap();

    let instructions = [
        system_instruction::create_account(
            &payer.pubkey(),
            &keypair.pubkey(),
            rent,
            spl_token::state::Account::LEN as u64,
            &spl_token::id(),
        ),
        spl_token::instruction::initialize_account(
            &spl_token::id(),
            &keypair.pubkey(),
            &mint,
            owner,
        )
        .unwrap(),
    ];

    process_transaction(rpc_client, &instructions, payer, Some(&[&keypair]))
        .await
        .unwrap();
    return keypair.pubkey();
}

#[allow(dead_code)]
pub async fn create_mint(
    rpc_client: &RpcClient,
    mint_keypair: &Keypair,
    mint_authority: &Keypair,
    freeze_authority: Option<&Pubkey>,
) -> Result<(), Box<dyn Error>> {
    let mint_rent = rpc_client
        .get_minimum_balance_for_rent_exemption(spl_token::state::Mint::LEN)
        .await?;

    let instructions = [
        system_instruction::create_account(
            &mint_authority.pubkey(),
            &mint_keypair.pubkey(),
            mint_rent,
            spl_token::state::Mint::LEN as u64,
            &spl_token::id(),
        ),
        spl_token::instruction::initialize_mint(
            &spl_token::id(),
            &mint_keypair.pubkey(),
            &mint_authority.pubkey(),
            freeze_authority,
            6,
        )
        .unwrap(),
    ];

    process_transaction(
        rpc_client,
        &instructions,
        mint_authority,
        Some(&[&mint_keypair]),
    )
    .await?;

    return Ok(());
}

#[allow(dead_code)]
pub async fn mint_tokens(
    rpc_client: &RpcClient,
    token_mint: &Pubkey,
    token_mint_authority: &Keypair,
    owner: &Pubkey,
    amount: u64,
) -> Result<(), Box<dyn Error>> {
    let create_ata_account =
        spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &token_mint_authority.pubkey(),
            owner,
            token_mint,
            &spl_token::id(),
        );

    let token_account =
        get_associated_token_address_with_program_id(owner, token_mint, &spl_token::id());
    let mint_instruction = spl_token::instruction::mint_to(
        &spl_token::id(),
        token_mint,
        &token_account,
        &token_mint_authority.pubkey(),
        &[],
        amount,
    )
    .unwrap();

    process_transaction(
        rpc_client,
        &[create_ata_account, mint_instruction],
        token_mint_authority,
        None,
    )
    .await?;

    return Ok(());
}

pub async fn get_account_data(rpc_client: &RpcClient, address: Pubkey) -> Vec<u8> {
    rpc_client
        .get_account(&address)
        .await
        .unwrap()
        .data
        .to_vec()
}

pub async fn get_account<T: AccountDeserialize>(rpc_client: &RpcClient, address: Pubkey) -> T {
    let data = get_account_data(rpc_client, address).await;
    let mut data_slice: &[u8] = &data;
    AccountDeserialize::try_deserialize(&mut data_slice).unwrap()
}

pub async fn token_account_balance(rpc_client: &RpcClient, address: Pubkey) -> u64 {
    get_account::<TokenAccount>(rpc_client, address)
        .await
        .amount
}
