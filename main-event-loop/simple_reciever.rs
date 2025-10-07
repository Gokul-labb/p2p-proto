//! Simple receiver example
//! 
//! Demonstrates how to run a P2P file converter in receiver mode.

use anyhow::Result;
use p2p_file_converter::prelude::*;
use std::env;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,libp2p=debug")
        .init();

    println!("🔄 P2P File Converter - Receiver Mode Example");

    // Parse command line args or use defaults
    let args = env::args().collect::<Vec<_>>();
    let listen_port = args.get(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);

    // Override CLI args programmatically
    env::set_var("P2P_LISTEN_PORT", listen_port.to_string());

    // Create configuration for receiver mode
    let config = FileConversionConfig {
        output_dir: std::path::PathBuf::from("./received_files"),
        auto_convert: true,
        return_results: false,
        max_concurrent_transfers: 3,
        pdf_config: PdfConfig {
            title: "Received Document".to_string(),
            font_size: 12,
            margins: 20,
            ..Default::default()
        },
    };

    // Create P2P node
    let mut node = P2PFileNode::new(config).await?;
    let listen_addr = format!("/ip4/0.0.0.0/tcp/{}", listen_port).parse()?;

    println!("🌐 Starting receiver on: {}", listen_addr);
    println!("📁 Files will be saved to: ./received_files/");
    println!("🔄 Auto-conversion enabled");
    println!("📋 Press Ctrl+C to stop");

    // Handle shutdown gracefully
    let node_task = tokio::spawn(async move {
        if let Err(e) = node.run(listen_addr).await {
            eprintln!("Node error: {}", e);
        }
    });

    // Wait for shutdown signal
    tokio::select! {
        _ = signal::ctrl_c() => {
            println!("\n🛑 Shutdown signal received");
        }
        _ = node_task => {
            println!("🔚 Node task completed");
        }
    }

    println!("👋 Receiver stopped");
    Ok(())
}
