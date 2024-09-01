#!/bin/bash

# Check if both .so file and keypair file are provided as arguments
if [ $# -ne 2 ]; then
    echo "Error: Incorrect number of arguments."
    echo "Usage: $0 <path_to_program.so> <path_to_keypair.json>"
    exit 1
fi

# Get the .so file path from the first argument
PROGRAM_SO=$1

# Get the keypair file path from the second argument
KEYPAIR_FILE=$2

# Check if the .so file exists
if [ ! -f "$PROGRAM_SO" ]; then
    echo "Error: Program file $PROGRAM_SO does not exist."
    exit 1
fi

# Check if the keypair file exists
if [ ! -f "$KEYPAIR_FILE" ]; then
    echo "Error: Keypair file $KEYPAIR_FILE does not exist."
    exit 1
fi

# Deploy the program to localnet using the specified keypair
echo "Deploying $PROGRAM_SO to localnet using keypair $KEYPAIR_FILE..."
solana program deploy "$PROGRAM_SO" --program-id "$KEYPAIR_FILE" -u l --upgrade-authority lifecycle-test/keypairs/auth-keypair.json

# Check if the deployment was successful
if [ $? -eq 0 ]; then
    echo "Program deployed successfully."
    
    # Get the program id
    PROGRAM_ID=$(solana address -k "$PROGRAM_SO")
    echo "Program ID: $PROGRAM_ID"
else
    echo "Program deployment failed."
    exit 1
fi