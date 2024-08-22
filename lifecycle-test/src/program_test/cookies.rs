use anchor_client::solana_sdk::signature::Keypair;
use anchor_lang::prelude::Pubkey;

use crate::program_test::utils::*;

pub struct UserCookie {
    pub key: Keypair,
    pub token_accounts: Vec<Pubkey>,
}
