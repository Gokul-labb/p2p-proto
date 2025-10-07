//! P2P File Converter - Main Binary
//! 
//! Command-line application for peer-to-peer file conversion and transfer.

use anyhow::Result;
use p2p_file_converter::main_event_loop::P2PFileConverter;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Print banner
    println!("🚀 P2P File Converter v{}", p2p_file_converter::VERSION);
    println!("   A peer-to-peer file conversion and transfer system");
    println!();

    // Create and run the application
    let mut app = P2PFileConverter::new().await?;
    let exit_code = app.run().await?;

    // Exit with appropriate code
    info!("Application finished with exit code: {}", exit_code);
    std::process::exit(exit_code);
}
