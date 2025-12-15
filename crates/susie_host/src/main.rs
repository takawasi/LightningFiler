//! Susie Plugin Bridge (32-bit)
//!
//! This binary runs as a separate 32-bit process to host legacy Susie plugins.
//! It communicates with the main 64-bit process via named pipes and shared memory.
//!
//! Build: cargo build --target i686-pc-windows-msvc -p susie_host

mod bridge;
mod susie;

use anyhow::Result;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer())
        .init();

    tracing::info!("Susie Bridge starting (32-bit)");

    // Run the bridge
    bridge::run()
}
