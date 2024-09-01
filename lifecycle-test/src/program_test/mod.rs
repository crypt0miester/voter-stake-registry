use anchor_client::solana_client::nonblocking::rpc_client::RpcClient;
use anchor_client::solana_sdk::signature::Keypair;
use anchor_lang::prelude::Pubkey;

pub use addin::*;
pub use governance::*;
pub use solana::*;

use crate::addin_lifecycle::delay_seconds;

pub mod addin;
pub mod governance;
pub mod solana;


pub struct Balances {
    pub token: u64,
    pub vault: u64,
    pub deposit: u64,
    pub voter_weight: u64,
}

pub async fn balances(
    rpc_client: &RpcClient,
    addin_cookie: &AddinCookie,
    registrar: &RegistrarCookie,
    address: Pubkey,
    voter: &VoterCookie,
    voting_mint: &VotingMintConfigCookie,
    payer: &Keypair,
    deposit_id: u8,
) -> Balances {
    delay_seconds(1).await;

    let token = token_account_balance(rpc_client, address).await;
    let vault = voting_mint.vault_balance(&rpc_client, &voter).await;
    let deposit = voter.deposit_amount(&rpc_client, deposit_id).await;
    let vwr = addin_cookie
        .update_voter_weight_record(rpc_client,&registrar, &voter, payer)
        .await
        .unwrap();
    Balances {
        token,
        vault,
        deposit,
        voter_weight: vwr.voter_weight,
    }
}
