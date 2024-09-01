use std::process::Command;
use anchor_client::solana_sdk::pubkey::Pubkey;
use std::error::Error;

pub async fn upgrade_spl_gov_program() -> Result<(), Box<dyn Error>> {
    // Run the Solana CLI command to upgrade the program
    let output = Command::new("./lifecycle-test/scripts/deploy_program_localnet.sh")
        .args(&[
            "lifecycle-test/fixtures/spl_governance_4.so",
            "lifecycle-test/keypairs/goverenance-program-keypair.json"
        ])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Failed to upgrade program: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    println!("Successfully upgraded spl-gov program to v4");

    Ok(())
}