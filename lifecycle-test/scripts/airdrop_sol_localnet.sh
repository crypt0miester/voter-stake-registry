#!/bin/bash

# Check if a keypair file is provided as an argument
if [ $# -eq 0 ]; then
    echo "Error: No keypair file specified."
    echo "Usage: $0 <path_to_keypair.json>"
    exit 1
fi

# Get the keypair file path from the first argument
KEYPAIR_FILE=$1

# Check if the file exists
if [ ! -f "$KEYPAIR_FILE" ]; then
    echo "Error: File $KEYPAIR_FILE does not exist."
    exit 1
fi

# 100 SOL
AMOUNT=100000000000

# Airdrop SOL to the specified keypair
echo "Airdropping 100 SOL to the keypair..."
solana airdrop $AMOUNT "$KEYPAIR_FILE" -u l

# Check if the airdrop was successful
if [ $? -eq 0 ]; then
    echo "Airdrop successful."
else
    echo "Airdrop failed."
    exit 1
fi

# Display the new balance
solana balance "$KEYPAIR_FILE"