// Besure AI Context — Library entry point
// Exposes core modules for both the CLI binary and the Tauri desktop app

pub mod crypto;
pub mod storage;
pub mod ai;
pub mod dashboard;

/// Embedded Dashboard HTML
pub const DASHBOARD_HTML: &str = include_str!("dashboard.html");
