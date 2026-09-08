//! User-configurable profile/settings, persisted as JSON in localStorage.
//!
//! The defaults are dummy placeholders: nothing about the original owner is
//! baked in. Whatever the user types in the Settings view is saved to
//! localStorage on every change and restored the next time the page loads.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

/// localStorage key under which the settings are persisted.
pub const STORAGE_KEY: &str = "gst-bill.config.v1";

/// Everything the user can configure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    // ---- Seller ("From") — the entity issuing the invoice ----
    pub seller_name: String,
    pub seller_address_1: String,
    pub seller_address_2: String,
    pub seller_pincode: String,
    pub seller_phone: String,
    pub seller_gstin: String,
    pub seller_state: String,
    pub seller_pan: String,

    // ---- Buyer ("To") ----
    pub buyer_name: String,
    pub buyer_address_1: String,
    pub buyer_address_2: String,
    pub buyer_website: String,
    pub place_of_supply: String,
    pub country_of_supply: String,

    // ---- Invoice defaults ----
    pub invoice_prefix: String,
    pub default_item: String,
    pub default_hsn: String,
    pub default_rate: String,
    pub declaration_text: String,
}

impl Default for Config {
    /// Dummy placeholder defaults (see module docs).
    fn default() -> Self {
        Self {
            seller_name: "Your Name".to_owned(),
            seller_address_1: "Address line 1".to_owned(),
            seller_address_2: "Address line 2".to_owned(),
            seller_pincode: "000000".to_owned(),
            seller_phone: "+91 00000 00000".to_owned(),
            seller_gstin: "99XXXXX0000X1Z5".to_owned(),
            seller_state: "99-Dummy State".to_owned(),
            seller_pan: "XXXXX0000X".to_owned(),

            buyer_name: "Your Client Pvt. Ltd.".to_owned(),
            buyer_address_1: "Client address".to_owned(),
            buyer_address_2: "Client city, country".to_owned(),
            buyer_website: "www.example.com".to_owned(),
            place_of_supply: "97-Other territory".to_owned(),
            country_of_supply: "Other Country".to_owned(),

            invoice_prefix: "INV".to_owned(),
            default_item: "Software Consulting".to_owned(),
            default_hsn: "998314".to_owned(),
            default_rate: "0".to_owned(),
            declaration_text: "SUPPLY MEANT FOR EXPORT UNDER BOND OR LETTER OF \
                               UNDERTAKING WITHOUT PAYMENT OF INTEGRATED TAX"
                .to_owned(),
        }
    }
}

impl Config {
    /// Read the stored settings. `Ok(None)` when nothing has been saved yet.
    pub fn load() -> Result<Option<Self>> {
        let storage = local_storage()?;
        let raw = storage
            .get_item(STORAGE_KEY)
            .map_err(|_| anyhow!("failed to read settings from localStorage"))?;
        match raw {
            None => Ok(None),
            Some(raw) => serde_json::from_str(&raw)
                .map(Some)
                .context("stored settings are not valid JSON"),
        }
    }

    /// Persist the settings to localStorage.
    pub fn save(&self) -> Result<()> {
        let storage = local_storage()?;
        let json = serde_json::to_string(self).context("failed to serialize settings")?;
        storage
            .set_item(STORAGE_KEY, &json)
            .map_err(|_| anyhow!("failed to write settings to localStorage"))?;
        Ok(())
    }

    /// Best-effort restore: saved settings when present and parseable, else
    /// the placeholder defaults. Never fails; logs a warning on error.
    pub fn load_or_default() -> Self {
        match Self::load() {
            Ok(Some(config)) => config,
            Ok(None) => Self::default(),
            Err(err) => {
                crate::log::warn(&format!(
                    "Could not restore settings, using placeholders: {err:#}"
                ));
                Self::default()
            }
        }
    }
}

fn local_storage() -> Result<web_sys::Storage> {
    let window = web_sys::window().ok_or_else(|| anyhow!("no window object available"))?;
    window
        .local_storage()
        .map_err(|_| anyhow!("failed to access localStorage"))?
        .ok_or_else(|| anyhow!("localStorage is not available in this browser"))
}
