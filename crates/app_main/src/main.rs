//! LightningFiler - High-Performance Image Filer/Viewer for Windows
//!
//! Main entry point for the 64-bit application.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use anyhow::Result;

fn main() -> Result<()> {
    // Initialize logging and panic hook first
    app_log::init()?;

    // Clean up old logs (7 days)
    if let Err(e) = app_log::cleanup_old_logs(7) {
        tracing::warn!("Failed to cleanup old logs: {}", e);
    }

    tracing::info!("LightningFiler starting...");

    // Load configuration
    let config = app_core::AppConfig::load().unwrap_or_default();

    // Initialize application state
    let _state = app_core::init(config)?;

    // Run the application
    app::run()
}
