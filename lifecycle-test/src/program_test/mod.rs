use anchor_client::solana_client::nonblocking::rpc_client::RpcClient;
use anchor_client::solana_sdk::signature::Keypair;
use anchor_lang::prelude::Pubkey;
use log::*;
use std::cell::RefCell;
use std::{str::FromStr, sync::Arc, sync::RwLock};

use solana_program::{program_option::COption, program_pack::Pack};
use spl_token::{state::*, *};

pub use vsr::*;
pub use cookies::*;
pub use governance::*;
pub use solana::*;
pub use utils::*;

pub mod vsr;
pub mod cookies;
pub mod governance;
pub mod solana;
pub mod utils;


pub struct Balances {
    token: u64,
    vault: u64,
    deposit: u64,
    voter_weight: u64,
}

pub async fn balances(
    rpc_client: &RpcClient,
    vsr_addin: &VsrCookie,
    registrar: &RegistrarCookie,
    address: Pubkey,
    voter: &VoterCookie,
    voting_mint: &VotingMintConfigCookie,
    payer: &Keypair,
    deposit_id: u8,
) -> Balances {
    // Advance slots to avoid caching of the UpdateVoterWeightRecord call
    // TODO: Is this something that could be an issue on a live node?
    // sleep

    let token = token_account_balance(rpc_client, address).await;
    let vault = voting_mint.vault_balance(&rpc_client, &voter).await;
    let deposit = voter.deposit_amount(&rpc_client, deposit_id).await;
    let vwr = vsr_addin
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
