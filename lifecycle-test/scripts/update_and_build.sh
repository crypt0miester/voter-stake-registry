#!/bin/bash

# Change to the project root directory
# cd ../../

# Update program ID
PROGRAM_ID=$(solana address -k lifecycle-test/keypairs/vsr-program-keypair.json)
sed -i "s/declare_id!(\"[^\"]*\")/declare_id!(\"$PROGRAM_ID\")/" programs/voter-stake-registry/src/lib.rs

# Update allowed_program pubkey in set_time_offset.rs
ALLOWED_PROGRAM=$(solana address -k lifecycle-test/keypairs/goverenance-program-keypair.json)
sed -i "s/Pubkey::from_str(\"[^\"]*\")/Pubkey::from_str(\"$ALLOWED_PROGRAM\")/" programs/voter-stake-registry/src/instructions/set_time_offset.rs

# Run anchor build with the program id changes
anchor build

# Revert changes
git checkout -- programs/voter-stake-registry/src/lib.rs programs/voter-stake-registry/src/instructions/set_time_offset.rs