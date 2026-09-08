//! Standalone-HTML export.
//!
//! The old CLI wrote self-contained `.html` files that were emailed to the
//! client. This module reproduces that artifact from the live preview: it
//! wraps the captured preview markup in a standalone document (Bootstrap CDN
//! included) and triggers a browser download.

use anyhow::{anyhow, Result};
use wasm_bindgen::JsCast;

use crate::logic::{month_name, percent_encode, Ymd};

const BOOTSTRAP_LINK: &str = r#"<link rel="stylesheet"
      href="https://stackpath.bootstrapcdn.com/bootstrap/4.4.1/css/bootstrap.min.css"
      integrity="sha384-Vkoo8x4CGsO3+Hhxv8T/Q5PaXtkKtu6ug5TOeNV6gBiFeWPGFN9MuhOf23Q9Ifjh"
      crossorigin="anonymous" />"#;

/// Wrap captured invoice markup (a bootstrap `.card`) into a full standalone
/// HTML document, mirroring the layout of the old CLI output.
pub fn wrap_standalone(sheet_html: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"UTF-8\" />\n    \
         <title>GST Bill</title>\n    {BOOTSTRAP_LINK}\n  </head>\n  <body>\n    \
         <div class=\"container\">\n{sheet_html}\n    </div>\n  </body>\n</html>\n"
    )
}

/// Suggested download name following the old convention:
/// <seller-slug>-<prefix>-<number>-<Month>.html
/// (the finance year is intentionally not part of the filename, as before).
pub fn filename(seller_name: &str, prefix: &str, invoice_no: &str, invoice_date: &str) -> String {
    let slug: String = seller_name
        .split_whitespace()
        .next()
        .unwrap_or("invoice")
        .to_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect();
    let slug = if slug.is_empty() { "invoice" } else { slug.as_str() };
    let prefix = prefix.trim().to_lowercase();
    let number = invoice_no.trim();
    let month = Ymd::parse(invoice_date).and_then(|date| month_name(date.month));
    match month {
        Some(month) => format!("{slug}-{prefix}-{number}-{month}.html"),
        None => format!("{slug}-{prefix}-{number}.html"),
    }
}

/// Download the current invoice preview (the element with id `sheet_id`) as a
/// standalone HTML file. Errors are returned to the caller for logging.
pub fn download(sheet_id: &str, seller_name: &str, prefix: &str, invoice_no: &str, invoice_date: &str) -> Result<()> {
    let window = web_sys::window().ok_or_else(|| anyhow!("no window object available"))?;
    let document = window
        .document()
        .ok_or_else(|| anyhow!("no document object available"))?;
    let sheet = document
        .get_element_by_id(sheet_id)
        .ok_or_else(|| anyhow!("invoice preview not found"))?;
    let html = wrap_standalone(&sheet.inner_html());
    let href = format!("data:text/html;charset=utf-8,{}", percent_encode(&html));

    let element = document
        .create_element("a")
        .map_err(|_| anyhow!("failed to create download link"))?;
    let anchor = element
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| anyhow!("failed to create download link"))?;
    anchor
        .set_attribute("href", &href)
        .map_err(|_| anyhow!("failed to set download href"))?;
    anchor
        .set_attribute(
            "download",
            &filename(seller_name, prefix, invoice_no, invoice_date),
        )
        .map_err(|_| anyhow!("failed to set download filename"))?;
    let body = document
        .body()
        .ok_or_else(|| anyhow!("no document body available"))?;
    body.append_child(&anchor)
        .map_err(|_| anyhow!("failed to attach download link"))?;
    anchor.click();
    anchor.remove();
    Ok(())
}
