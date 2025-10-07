// Simple usage example showing CLI argument parsing

use clap::Parser;
use p2p_file_converter_cli::{CliArgs, AppMode};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI arguments - this handles all validation automatically
    let (args, mode) = CliArgs::parse_args()?;

    // Setup logging
    args.setup_logging()?;

    // Print configuration
    args.print_config(&mode);

    // Handle different modes
    match mode {
        AppMode::Receiver { listen_addr, output_dir } => {
            println!("🎧 Receiver Mode Active");
            println!("   Listening on: {}", listen_addr);
            println!("   Saving files to: {}", output_dir.display());
        }
        AppMode::Sender { target_addr, file_path, .. } => {
            println!("📤 Sender Mode Active"); 
            println!("   Sending: {}", file_path.display());
            println!("   To peer: {}", target_addr);
        }
    }

    Ok(())
}
