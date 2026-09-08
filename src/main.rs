//! GST Bill — fully client-side (CSR) Leptos app compiled to WASM.
//!
//! There is no server: the page mounts the app, reads/saves settings through
//! the browser's localStorage, and exports invoices via print/PDF or a
//! standalone HTML download.

mod app;
mod config;
mod export;
mod log;
mod logic;

use app::App;
use leptos::prelude::*;

/// Browser entry point (runs once the WASM module loads).
fn main() {
    if let Err(err) = run() {
        log::error(&format!("Failed to start: {err:#}"));
    }
}

/// Fallible startup. Errors propagate here as `anyhow` values instead of
/// panicking; the error is reported to the console and the page stays inert
/// rather than crashing mid-render.
fn run() -> anyhow::Result<()> {
    mount_to_body(|| view! { <App/> });
    Ok(())
}
