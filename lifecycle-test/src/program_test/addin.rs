use anchor_client::solana_sdk::signature::Keypair;
use anchor_client::solana_sdk::signer::Signer;
use anchor_lang::system_program;
use anchor_spl::token::TokenAccount;
use program_test::{get_account, process_transaction, GovernanceRealmCookie};
use solana_program::sysvar::rent;
use spl_token::solana_program::instruction::Instruction;
use crate::program_test::TokenOwnerRecordCookie;

use crate::*;

#[derive(Clone)]
pub struct AddinCookie {
    pub program_id: Pubkey,
}

pub struct RegistrarCookie {
    pub address: Pubkey,
    pub authority: Pubkey,
    pub mint: Pubkey,
}

#[derive(Clone)]
pub struct VotingMintConfigCookie {
    pub mint: Pubkey,
}

pub struct VoterCookie {
    pub address: Pubkey,
    pub authority: Pubkey,
    pub voter_weight_record: Pubkey,
    pub token_owner_record: Pubkey,
}

impl AddinCookie {
    pub async fn create_registrar(
        &self,
        rpc_client: &RpcClient,
        realm: &GovernanceRealmCookie,
        authority: &Keypair,
        payer: &Keypair,
    ) -> RegistrarCookie {
        let community_token_mint = realm.community_token_mint;

        let (registrar, registrar_bump) = Pubkey::find_program_address(
            &[
                &realm.realm.to_bytes(),
                b"registrar".as_ref(),
                &community_token_mint.to_bytes(),
            ],
            &self.program_id,
        );

        let data = anchor_lang::InstructionData::data(
            &voter_stake_registry::instruction::CreateRegistrar { registrar_bump },
        );

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::CreateRegistrar {
                registrar,
                governance_program_id: realm.governance.program_id,
                realm: realm.realm,
                realm_governing_token_mint: community_token_mint,
                realm_authority: realm.authority,
                payer: payer.pubkey(),
                system_program: system_program::ID,
                rent: rent::id(),
            },
            None,
        );

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the user secret
        let signer1 = Keypair::from_base58_string(&payer.to_base58_string());
        let signer2 = Keypair::from_base58_string(&authority.to_base58_string());

        process_transaction(
            rpc_client,
            &instructions,
            payer,
            Some(&[&signer1, &signer2]),
        )
        .await
        .unwrap();

        RegistrarCookie {
            address: registrar,
            authority: realm.authority,
            mint: realm.community_token_mint.clone(),
        }
    }

    pub async fn configure_voting_mint(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        authority: &Keypair,
        payer: &Keypair,
        index: u16,
        mint: &Pubkey,
        digit_shift: i8,
        baseline_vote_weight_scaled_factor: f64,
        max_extra_lockup_vote_weight_scaled_factor: f64,
        lockup_saturation_secs: u64,
        grant_authority: Option<Pubkey>,
        other_mints: Option<&[Pubkey]>,
    ) -> VotingMintConfigCookie {
        let deposit_mint = *mint;

        let data = anchor_lang::InstructionData::data(
            &voter_stake_registry::instruction::ConfigureVotingMint {
                idx: index,
                digit_shift,
                baseline_vote_weight_scaled_factor: (baseline_vote_weight_scaled_factor * 1e9)
                    as u64,
                max_extra_lockup_vote_weight_scaled_factor:
                    (max_extra_lockup_vote_weight_scaled_factor * 1e9) as u64,
                lockup_saturation_secs,
                grant_authority,
            },
        );

        let mut accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::ConfigureVotingMint {
                mint: deposit_mint,
                registrar: registrar.address,
                realm_authority: authority.pubkey(),
            },
            None,
        );
        accounts.push(anchor_lang::prelude::AccountMeta::new_readonly(
            deposit_mint,
            false,
        ));
        for mint in other_mints.unwrap_or(&[]) {
            accounts.push(anchor_lang::prelude::AccountMeta::new_readonly(
                *mint, false,
            ));
        }

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the user secret
        //let signer1 = Keypair::from_base58_string(&payer.to_base58_string());
        let signer2 = Keypair::from_base58_string(&authority.to_base58_string());

        process_transaction(
            rpc_client,
            &instructions,
            payer,
            Some(&[&signer2]),
        )
        .await
        .unwrap();

        VotingMintConfigCookie { mint: mint.clone() }
    }

    pub async fn create_voter(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        token_owner_record: &TokenOwnerRecordCookie,
        authority: &Keypair,
        payer: &Keypair,
    ) -> VoterCookie {
        let (voter, voter_bump) = Pubkey::find_program_address(
            &[
                &registrar.address.to_bytes(),
                b"voter".as_ref(),
                &authority.pubkey().to_bytes(),
            ],
            &self.program_id,
        );
        let (voter_weight_record, voter_weight_record_bump) = Pubkey::find_program_address(
            &[
                &registrar.address.to_bytes(),
                b"voter-weight-record".as_ref(),
                &authority.pubkey().to_bytes(),
            ],
            &self.program_id,
        );

        let data =
            anchor_lang::InstructionData::data(&voter_stake_registry::instruction::CreateVoter {
                voter_bump,
                voter_weight_record_bump,
            });

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::CreateVoter {
                voter,
                voter_weight_record,
                registrar: registrar.address,
                voter_authority: authority.pubkey(),
                payer: payer.pubkey(),
                system_program: system_program::ID,
                rent: solana_program::sysvar::rent::id(),
                instructions: solana_program::sysvar::instructions::id(),
            },
            None,
        );

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the secrets
        let signer1 = Keypair::from_base58_string(&payer.to_base58_string());
        let signer2 = Keypair::from_base58_string(&authority.to_base58_string());

        process_transaction(&rpc_client, &instructions, payer, Some(&[&signer1, &signer2]))
            .await
            .unwrap();

        VoterCookie {
            address: voter,
            authority: authority.pubkey(),
            voter_weight_record,
            token_owner_record: token_owner_record.address,
        }
    }

    pub async fn create_deposit_entry(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        voter: &VoterCookie,
        voter_authority: &Keypair,
        voting_mint: &VotingMintConfigCookie,
        deposit_entry_index: u8,
        lockup_kind: voter_stake_registry::state::LockupKind,
        start_ts: Option<u64>,
        periods: u32,
        allow_clawback: bool,
    ) -> Result<(), Box<dyn Error>> {
        let vault = voter.vault_address(&voting_mint);

        let data = anchor_lang::InstructionData::data(
            &voter_stake_registry::instruction::CreateDepositEntry {
                deposit_entry_index,
                kind: lockup_kind,
                start_ts,
                periods,
                allow_clawback,
            },
        );

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::CreateDepositEntry {
                vault,
                registrar: registrar.address,
                voter: voter.address,
                voter_authority: voter_authority.pubkey(),
                payer: voter_authority.pubkey(),
                deposit_mint: voting_mint.mint,
                system_program: system_program::ID,
                token_program: spl_token::id(),
                associated_token_program: spl_associated_token_account::id(),
                rent: solana_program::sysvar::rent::id(),
            },
            None,
        );

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the secrets
        let signer = Keypair::from_base58_string(&voter_authority.to_base58_string());

        process_transaction(rpc_client,&instructions, voter_authority, Some(&[&signer]))
            .await
    }

    #[allow(dead_code)]
    pub async fn deposit(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        voter: &VoterCookie,
        voting_mint: &VotingMintConfigCookie,
        authority: &Keypair,
        token_address: Pubkey,
        deposit_entry_index: u8,
        amount: u64,
    ) -> Result<(), Box<dyn Error>> {
        let vault = voter.vault_address(&voting_mint);

        let data =
            anchor_lang::InstructionData::data(&voter_stake_registry::instruction::Deposit {
                deposit_entry_index,
                amount,
            });

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::Deposit {
                registrar: registrar.address,
                voter: voter.address,
                vault: vault,
                deposit_token: token_address,
                deposit_authority: authority.pubkey(),
                token_program: spl_token::id(),
            },
            None,
        );

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the secrets
        let signer = Keypair::from_base58_string(&authority.to_base58_string());

        process_transaction(rpc_client,&instructions, authority, Some(&[&signer]))
            .await
    }

    #[allow(dead_code)]
    pub async fn grant(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        voter_authority: Pubkey,
        voting_mint: &VotingMintConfigCookie,
        lockup_kind: voter_stake_registry::state::LockupKind,
        start_ts: Option<u64>,
        periods: u32,
        allow_clawback: bool,
        amount: u64,
        deposit_token: Pubkey,
        token_authority: &Keypair,
        grant_authority: &Keypair,
    ) -> Result<VoterCookie, Box<dyn Error>> {
        let (voter, voter_bump) = Pubkey::find_program_address(
            &[
                &registrar.address.to_bytes(),
                b"voter".as_ref(),
                &voter_authority.to_bytes(),
            ],
            &self.program_id,
        );
        let (voter_weight_record, voter_weight_record_bump) = Pubkey::find_program_address(
            &[
                &registrar.address.to_bytes(),
                b"voter-weight-record".as_ref(),
                &voter_authority.to_bytes(),
            ],
            &self.program_id,
        );
        let voter_cookie = VoterCookie {
            address: voter,
            authority: voter_authority,
            voter_weight_record,
            token_owner_record: Pubkey::new_unique(), // don't have it
        };
        let vault = voter_cookie.vault_address(&voting_mint);

        let data = anchor_lang::InstructionData::data(&voter_stake_registry::instruction::Grant {
            voter_bump,
            voter_weight_record_bump,
            kind: lockup_kind,
            start_ts,
            periods,
            allow_clawback,
            amount,
        });

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::Grant {
                registrar: registrar.address,
                voter,
                voter_authority,
                voter_weight_record,
                vault,
                deposit_token,
                token_authority: token_authority.pubkey(),
                grant_authority: grant_authority.pubkey(),
                payer: token_authority.pubkey(),
                deposit_mint: voting_mint.mint,
                system_program: system_program::ID,
                token_program: spl_token::id(),
                associated_token_program: spl_associated_token_account::id(),
                rent: solana_program::sysvar::rent::id(),
            },
            None,
        );

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the secrets
        let signer1 = Keypair::from_base58_string(&grant_authority.to_base58_string());
        let signer2 = Keypair::from_base58_string(&token_authority.to_base58_string());

        process_transaction(rpc_client,&instructions, grant_authority, Some(&[&signer1, &signer2]))
            .await?;

        Ok(voter_cookie)
    }

    #[allow(dead_code)]
    pub async fn clawback(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        voter: &VoterCookie,
        voting_mint: &VotingMintConfigCookie,
        realm_authority: &Keypair,
        token_address: Pubkey,
        deposit_entry_index: u8,
    ) -> Result<(), Box<dyn Error>> {
        let vault = voter.vault_address(&voting_mint);

        let data =
            anchor_lang::InstructionData::data(&voter_stake_registry::instruction::Clawback {
                deposit_entry_index,
            });

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::Clawback {
                registrar: registrar.address,
                voter: voter.address,
                vault,
                destination: token_address,
                realm_authority: realm_authority.pubkey(),
                token_program: spl_token::id(),
            },
            None,
        );

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the secrets
        let signer = Keypair::from_base58_string(&realm_authority.to_base58_string());

        process_transaction(rpc_client,&instructions, realm_authority, Some(&[&signer]))
            .await
    }

    #[allow(dead_code)]
    pub async fn withdraw(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        voter: &VoterCookie,
        voting_mint: &VotingMintConfigCookie,
        authority: &Keypair,
        token_address: Pubkey,
        deposit_entry_index: u8,
        amount: u64,
    ) -> Result<(), Box<dyn Error>> {
        let vault = voter.vault_address(&voting_mint);

        let data =
            anchor_lang::InstructionData::data(&voter_stake_registry::instruction::Withdraw {
                deposit_entry_index,
                amount,
            });

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::Withdraw {
                registrar: registrar.address,
                voter: voter.address,
                token_owner_record: voter.token_owner_record,
                voter_weight_record: voter.voter_weight_record,
                vault,
                destination: token_address,
                voter_authority: authority.pubkey(),
                token_program: spl_token::id(),
            },
            None,
        );

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the secrets
        let signer = Keypair::from_base58_string(&authority.to_base58_string());

        process_transaction(rpc_client,&instructions, authority, Some(&[&signer]))
            .await
    }

    #[allow(dead_code)]
    pub async fn close_voter(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        voter: &VoterCookie,
        voting_mint: &VotingMintConfigCookie,
        voter_authority: &Keypair,
    ) -> Result<(), Box<dyn Error>> {
        let vault = voter.vault_address(&voting_mint);

        let data =
            anchor_lang::InstructionData::data(&voter_stake_registry::instruction::CloseVoter {});

        let mut accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::CloseVoter {
                registrar: registrar.address,
                voter: voter.address,
                voter_authority: voter_authority.pubkey(),
                sol_destination: voter_authority.pubkey(),
                token_program: spl_token::id(),
            },
            None,
        );
        accounts.push(anchor_lang::prelude::AccountMeta::new(vault, false));

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the secrets
        let signer = Keypair::from_base58_string(&voter_authority.to_base58_string());

        process_transaction(rpc_client,&instructions, voter_authority, Some(&[&signer]))
            .await
    }

    pub fn update_voter_weight_record_instruction(
        &self,
        registrar: &RegistrarCookie,
        voter: &VoterCookie,
    ) -> Instruction {
        let data = anchor_lang::InstructionData::data(
            &voter_stake_registry::instruction::UpdateVoterWeightRecord {},
        );

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::UpdateVoterWeightRecord {
                registrar: registrar.address,
                voter: voter.address,
                voter_weight_record: voter.voter_weight_record,
                system_program: system_program::ID,
            },
            None,
        );

        Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }
    }

    #[allow(dead_code)]
    pub async fn update_voter_weight_record(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        voter: &VoterCookie,
        payer: &Keypair,
    ) -> Result<voter_stake_registry::state::VoterWeightRecord, Box<dyn Error>> {
        let instructions = vec![self.update_voter_weight_record_instruction(registrar, voter)];

        process_transaction(rpc_client, &instructions,payer, None).await?;

        Ok(get_account::<voter_stake_registry::state::VoterWeightRecord>(
            rpc_client,
                voter.voter_weight_record,
            )
            .await)
    }

    #[allow(dead_code)]
    pub async fn close_deposit_entry(
        &self,
        rpc_client: &RpcClient,
        voter: &VoterCookie,
        authority: &Keypair,
        deposit_entry_index: u8,
    ) -> Result<(), Box<dyn Error>> {
        let data = anchor_lang::InstructionData::data(
            &voter_stake_registry::instruction::CloseDepositEntry {
                deposit_entry_index,
            },
        );

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::CloseDepositEntry {
                voter: voter.address,
                voter_authority: authority.pubkey(),
            },
            None,
        );

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the secrets
        let signer = Keypair::from_base58_string(&authority.to_base58_string());

        process_transaction(rpc_client,&instructions, authority, Some(&[&signer]))
            .await
    }

    #[allow(dead_code)]
    pub async fn reset_lockup(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        voter: &VoterCookie,
        authority: &Keypair,
        deposit_entry_index: u8,
        kind: voter_stake_registry::state::LockupKind,
        periods: u32,
    ) -> Result<(), Box<dyn Error>> {
        let data =
            anchor_lang::InstructionData::data(&voter_stake_registry::instruction::ResetLockup {
                deposit_entry_index,
                kind,
                periods,
            });

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::ResetLockup {
                registrar: registrar.address,
                voter: voter.address,
                voter_authority: authority.pubkey(),
            },
            None,
        );

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the secrets
        let signer = Keypair::from_base58_string(&authority.to_base58_string());

        process_transaction(rpc_client,&instructions, authority, Some(&[&signer]))
            .await
    }

    #[allow(dead_code)]
    pub async fn internal_transfer_locked(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        voter: &VoterCookie,
        authority: &Keypair,
        source_deposit_entry_index: u8,
        target_deposit_entry_index: u8,
        amount: u64,
    ) -> Result<(), Box<dyn Error>> {
        let data = anchor_lang::InstructionData::data(
            &voter_stake_registry::instruction::InternalTransferLocked {
                source_deposit_entry_index,
                target_deposit_entry_index,
                amount,
            },
        );

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::InternalTransferLocked {
                registrar: registrar.address,
                voter: voter.address,
                voter_authority: authority.pubkey(),
            },
            None,
        );

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the secrets
        let signer = Keypair::from_base58_string(&authority.to_base58_string());

        process_transaction(rpc_client,&instructions, authority, Some(&[&signer]))
            .await
    }

    #[allow(dead_code)]
    pub async fn internal_transfer_unlocked(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        voter: &VoterCookie,
        authority: &Keypair,
        source_deposit_entry_index: u8,
        target_deposit_entry_index: u8,
        amount: u64,
    ) -> Result<(), Box<dyn Error>> {
        let data = anchor_lang::InstructionData::data(
            &voter_stake_registry::instruction::InternalTransferUnlocked {
                source_deposit_entry_index,
                target_deposit_entry_index,
                amount,
            },
        );

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::InternalTransferUnlocked {
                registrar: registrar.address,
                voter: voter.address,
                voter_authority: authority.pubkey(),
            },
            None,
        );

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the secrets
        let signer = Keypair::from_base58_string(&authority.to_base58_string());

        process_transaction(rpc_client,&instructions,authority, Some(&[&signer]))
            .await
    }

    #[allow(dead_code)]
    pub async fn log_voter_info(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        voter: &VoterCookie,
        payer: &Keypair,
        deposit_entry_begin: u8,
    ) {
        let data =
            anchor_lang::InstructionData::data(&voter_stake_registry::instruction::LogVoterInfo {
                deposit_entry_begin,
                deposit_entry_count: 8,
            });

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::LogVoterInfo {
                registrar: registrar.address,
                voter: voter.address,
            },
            None,
        );

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        process_transaction(rpc_client,&instructions,payer, None)
            .await
            .unwrap();
    }

    #[allow(dead_code)]
    pub async fn set_time_offset(
        &self,
        rpc_client: &RpcClient,
        registrar: &RegistrarCookie,
        authority: &Keypair,
        time_offset: i64,
    ) {
        let data =
            anchor_lang::InstructionData::data(&voter_stake_registry::instruction::SetTimeOffset {
                time_offset,
            });

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &voter_stake_registry::accounts::SetTimeOffset {
                registrar: registrar.address,
                realm_authority: authority.pubkey(),
            },
            None,
        );

        let instructions = vec![Instruction {
            program_id: self.program_id,
            accounts,
            data,
        }];

        // clone the secrets
        let signer = Keypair::from_base58_string(&authority.to_base58_string());

        
        process_transaction(rpc_client,&instructions, authority, Some(&[&signer]))
            .await
            .unwrap();
    }
}

impl VotingMintConfigCookie {
    #[allow(dead_code)]
    pub async fn vault_balance(&self, rpc_client: &RpcClient, voter: &VoterCookie) -> u64 {
        let vault = voter.vault_address(&self);
        crate::program_test::solana::get_account::<TokenAccount>(rpc_client, vault)
            .await
            .amount
    }
}

impl VoterCookie {
    #[allow(dead_code)]
    pub async fn deposit_amount(&self, rpc_client: &RpcClient, deposit_id: u8) -> u64 {
        crate::program_test::solana::get_account::<voter_stake_registry::state::Voter>(
            rpc_client,
            self.address,
        )
        .await
        .deposits[deposit_id as usize]
            .amount_deposited_native
    }

    pub fn vault_address(&self, mint: &VotingMintConfigCookie) -> Pubkey {
        spl_associated_token_account::get_associated_token_address(
            &self.address,
            &mint.mint,
        )
    }
}
