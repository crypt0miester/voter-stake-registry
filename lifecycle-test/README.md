# SPL-Gov Plugin Lifecycle Test - Vote Stake Registry. (draft readme)

Make sure you hve solana-validator-test running and anchor v0.30.1 installed
1. run `./lifecycle-test/scripts/update_and_build.sh`
2. run `./lifecycle-test/scripts/deploy_program_localnet.sh target/deploy/voter_stake_registry.so lifecycle-test/keypairs/vsr-program-keypair.json && ./lifecycle-test/scripts/deploy_program_localnet.sh lifecycle-test/fixtures/spl_governance.so lifecycle-test/keypairs/goverenance-program-keypair.json`
3. run vsr test `cargo run -p vsr-lifecycle --release`

