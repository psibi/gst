//! UI + state wiring for the GST bill generator (Leptos 0.8, CSR).
//!
//! State lives in `RwSignal`s created at the top of [`App`]:
//!   * `cfg`    — the persisted, configurable profile (see [`crate::config`])
//!   * `fields` — the working invoice (dates, number, line item, hours, rate)
//!
//! Nothing here panics: every fallible operation (localStorage, serialization,
//! DOM download) is handled through `anyhow::Result` and logged.

use leptos::prelude::*;

use crate::config::Config;
use crate::log::{error as console_error, warn as console_warn};
use crate::logic::{
    days_in_month, finance_month_index, finance_year, fmt_amount, input_value,
    previous_month_clamped, textarea_value, total_amount, Ymd,
};

// ---------------------------------------------------------------------------
// Top-level state
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum AppView {
    Invoice,
    Settings,
}

/// What a brand-new invoice looks like before the user edits it. Date-derived
/// fields are guessed from today; item/rate come from the configured defaults.
struct Draft {
    date: String,
    due: String,
    no: String,
    reference: String,
    hours: String,
    rate: String,
    item: String,
    hsn: String,
}

/// Guess the defaults for a fresh invoice, based on the current date and the
/// configured defaults:
///   * invoice date = same day last month (clamped to month end)
///   * due date     = last day of the current month
///   * invoice no   = month index of the invoice date in the financial year
fn guess_draft(cfg: &Config) -> Draft {
    match Ymd::today() {
        Ok(today) => {
            let date = previous_month_clamped(today);
            let due = Ymd {
                year: today.year,
                month: today.month,
                day: days_in_month(today.year, today.month),
            };
            let no = finance_month_index(date.month)
                .map(|index| index.to_string())
                .unwrap_or_default();
            Draft {
                date: date.fmt(),
                due: due.fmt(),
                no,
                reference: String::new(),
                hours: String::new(),
                rate: cfg.default_rate.clone(),
                item: cfg.default_item.clone(),
                hsn: cfg.default_hsn.clone(),
            }
        }
        Err(err) => {
            console_warn(&format!(
                "Could not read the clock to guess dates: {err:#}"
            ));
            Draft {
                date: String::new(),
                due: String::new(),
                no: String::new(),
                reference: String::new(),
                hours: String::new(),
                rate: cfg.default_rate.clone(),
                item: cfg.default_item.clone(),
                hsn: cfg.default_hsn.clone(),
            }
        }
    }
}

/// The working invoice. Signals are shared (Copy) and owned by [`App`], so
/// switching between the Invoice/Settings views never loses input.
#[derive(Clone, Copy)]
struct InvoiceFields {
    date: RwSignal<String>,
    due: RwSignal<String>,
    /// invoice number (free text, guessed by default)
    no: RwSignal<String>,
    /// true once the user types the number by hand, disabling auto-guessing
    no_manual: RwSignal<bool>,
    reference: RwSignal<String>,
    hours: RwSignal<String>,
    rate: RwSignal<String>,
    item: RwSignal<String>,
    hsn: RwSignal<String>,
}

impl InvoiceFields {
    fn new(draft: &Draft) -> Self {
        Self {
            date: RwSignal::new(draft.date.clone()),
            due: RwSignal::new(draft.due.clone()),
            no: RwSignal::new(draft.no.clone()),
            no_manual: RwSignal::new(false),
            reference: RwSignal::new(draft.reference.clone()),
            hours: RwSignal::new(draft.hours.clone()),
            rate: RwSignal::new(draft.rate.clone()),
            item: RwSignal::new(draft.item.clone()),
            hsn: RwSignal::new(draft.hsn.clone()),
        }
    }

    /// Replace the whole invoice with a fresh draft.
    fn apply(&self, draft: &Draft) {
        self.date.set(draft.date.clone());
        self.due.set(draft.due.clone());
        self.no.set(draft.no.clone());
        self.no_manual.set(false);
        self.reference.set(draft.reference.clone());
        self.hours.set(draft.hours.clone());
        self.rate.set(draft.rate.clone());
        self.item.set(draft.item.clone());
        self.hsn.set(draft.hsn.clone());
    }

    /// Re-guess the invoice number from the current invoice date, but only
    /// while the user has not typed a number by hand.
    fn reguess_no_if_automatic(&self) {
        if self.no_manual.get() {
            return;
        }
        let index = Ymd::parse(&self.date.get()).and_then(|date| finance_month_index(date.month));
        if let Some(index) = index {
            self.no.set(index.to_string());
        }
    }
}

/// Display form of the invoice number: `{prefix}-{finance year}-{number}`,
/// e.g. `FPCO-2026-5`. The finance year always follows the invoice date.
fn full_invoice_no(cfg: RwSignal<Config>, fields: InvoiceFields) -> Memo<String> {
    Memo::new(move |_| {
        let mut parts: Vec<String> = Vec::with_capacity(3);
        let prefix = cfg.get().invoice_prefix.trim().to_owned();
        if !prefix.is_empty() {
            parts.push(prefix);
        }
        if let Some(fy) = Ymd::parse(&fields.date.get())
            .and_then(|date| finance_year(date.year, date.month))
        {
            parts.push(fy.to_string());
        }
        let number = fields.no.get();
        if !number.is_empty() {
            parts.push(number);
        }
        parts.join("-")
    })
}

/// Persist the profile to localStorage, logging instead of panicking.
fn persist_cfg(cfg: RwSignal<Config>) {
    if let Err(err) = cfg.get_untracked().save() {
        console_warn(&format!("Could not save settings: {err:#}"));
    }
}

/// Handler factory for one text profile field: stores the value and saves.
fn bind_cfg(
    cfg: RwSignal<Config>,
    set: fn(&mut Config, String),
) -> impl Fn(web_sys::Event) + Send + Sync + 'static {
    move |ev| {
        let value = input_value(&ev);
        cfg.update(|config| set(config, value));
        persist_cfg(cfg);
    }
}

/// Handler factory for one multi-line textarea profile field.
fn bind_cfg_textarea(
    cfg: RwSignal<Config>,
    set: fn(&mut Config, String),
) -> impl Fn(web_sys::Event) + Send + Sync + 'static {
    move |ev| {
        let value = textarea_value(&ev);
        cfg.update(|config| set(config, value));
        persist_cfg(cfg);
    }
}

/// Handler factory for one working-invoice field.
fn bind_field(signal: RwSignal<String>) -> impl Fn(web_sys::Event) + Send + Sync + 'static {
    move |ev| signal.set(input_value(&ev))
}

// ---------------------------------------------------------------------------
// Reusable small form bits
// ---------------------------------------------------------------------------

/// A labelled one-line text/number/date input bound to a signal.
fn form_input(
    label: &'static str,
    input_type: &'static str,
    value: RwSignal<String>,
    on_input: impl Fn(web_sys::Event) + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="form-group">
            <label class="small text-muted mb-1">{label}</label>
            <input
                type=input_type
                class="form-control form-control-sm"
                prop:value=move || value.get()
                on:input=on_input
            />
        </div>
    }
}

/// A labelled one-line text input for a profile (settings) field.
fn text_input_group(
    label: &'static str,
    get: impl Fn() -> String + Send + Sync + 'static,
    on_input: impl Fn(web_sys::Event) + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="form-group">
            <label class="small text-muted mb-1">{label}</label>
            <input
                type="text"
                class="form-control form-control-sm"
                prop:value=get
                on:input=on_input
            />
        </div>
    }
}

/// A labelled multi-line textarea for a profile (settings) field.
fn textarea_group(
    label: &'static str,
    get: impl Fn() -> String + Send + Sync + 'static,
    on_input: impl Fn(web_sys::Event) + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="form-group">
            <label class="small text-muted mb-1">{label}</label>
            <textarea
                class="form-control form-control-sm"
                rows=3
                prop:value=get
                on:input=on_input
            ></textarea>
        </div>
    }
}

/// A labelled inline input with an extra trailing button (e.g. "auto").
fn form_input_with_button(
    label: &'static str,
    input_type: &'static str,
    button_label: &'static str,
    value: RwSignal<String>,
    on_input: impl Fn(web_sys::Event) + Send + Sync + 'static,
    on_button: impl Fn(web_sys::MouseEvent) + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <div class="form-group">
            <label class="small text-muted mb-1">{label}</label>
            <div class="input-group input-group-sm">
                <input
                    type=input_type
                    class="form-control"
                    prop:value=move || value.get()
                    on:input=on_input
                />
                <div class="input-group-append">
                    <button class="btn btn-outline-secondary" type="button" on:click=on_button>
                        {button_label}
                    </button>
                </div>
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// App shell
// ---------------------------------------------------------------------------

#[component]
pub fn App() -> impl IntoView {
    let view = RwSignal::new(AppView::Invoice);
    let cfg = RwSignal::new(Config::load_or_default());
    let fields = InvoiceFields::new(&guess_draft(&cfg.get_untracked()));

    let invoice_active =
        move || if view.get() == AppView::Invoice { "active" } else { "" };
    let settings_active =
        move || if view.get() == AppView::Settings { "active" } else { "" };

    view! {
        <div class="container-fluid py-3">
            <div class="no-print d-flex align-items-center border-bottom pb-2 mb-3">
                <h1 class="h5 mb-0 mr-4">"GST Bill Generator"</h1>
                <ul class="nav nav-pills">
                    <li class="nav-item">
                        <a
                            class=move || format!("nav-link {}", invoice_active())
                            href="#"
                            on:click=move |ev| { ev.prevent_default(); view.set(AppView::Invoice); }
                        >
                            "Invoice"
                        </a>
                    </li>
                    <li class="nav-item">
                        <a
                            class=move || format!("nav-link {}", settings_active())
                            href="#"
                            on:click=move |ev| { ev.prevent_default(); view.set(AppView::Settings); }
                        >
                            "Settings"
                        </a>
                    </li>
                </ul>
            </div>

            <Show when=move || view.get() == AppView::Invoice>
                <InvoiceWorkspace cfg=cfg fields=fields/>
            </Show>
            <Show when=move || view.get() == AppView::Settings>
                <SettingsWorkspace cfg=cfg/>
            </Show>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Invoice tab: entry form + live preview + export actions
// ---------------------------------------------------------------------------

#[component]
fn InvoiceWorkspace(cfg: RwSignal<Config>, fields: InvoiceFields) -> impl IntoView {
    let on_new = move |_| fields.apply(&guess_draft(&cfg.get_untracked()));

    let on_print = move |_| {
        match web_sys::window() {
            Some(window) => {
                let _ = window.print();
            }
            None => console_warn("print() unavailable: no window"),
        }
    };

    let on_download = move |_| {
        let config = cfg.get_untracked();
        let number = fields.no.get_untracked();
        let date = fields.date.get_untracked();
        if let Err(err) = crate::export::download(
            "invoice-sheet",
            &config.seller_name,
            &config.invoice_prefix,
            &number,
            &date,
        ) {
            console_error(&format!("Could not download invoice: {err:#}"));
        }
    };

    let total = Memo::new(move |_| total_amount(&fields.hours.get(), &fields.rate.get()));

    let on_date_input = move |ev: web_sys::Event| {
        let value = input_value(&ev);
        fields.date.set(value);
        fields.reguess_no_if_automatic();
    };
    let on_no_input = move |ev: web_sys::Event| {
        let value = input_value(&ev);
        fields.no.set(value);
        fields.no_manual.set(true);
    };
    let on_no_auto = move |_ev: web_sys::MouseEvent| {
        fields.no_manual.set(false);
        fields.reguess_no_if_automatic();
    };

    view! {
        <div>
            <div class="row mb-3 no-print">
                <div class="col">
                    <button class="btn btn-outline-secondary btn-sm mr-2" on:click=on_new>
                        "New invoice"
                    </button>
                    <button class="btn btn-outline-secondary btn-sm mr-2" on:click=on_print>
                        "Print / Save as PDF"
                    </button>
                    <button class="btn btn-outline-secondary btn-sm" on:click=on_download>
                        "Download HTML"
                    </button>
                </div>
            </div>

            <div class="row">
                <div class="col-lg-5 mb-3 no-print">
                    <div class="card">
                        <div class="card-header py-2">
                            <strong>"Invoice details"</strong>
                        </div>
                        <div class="card-body">
                            <div class="row">
                                <div class="col-sm-6">
                                    {form_input("Invoice date", "date", fields.date, on_date_input)}
                                </div>
                                <div class="col-sm-6">
                                    {form_input("Due date", "date", fields.due, bind_field(fields.due))}
                                </div>
                            </div>
                            {form_input_with_button(
                                "Invoice number",
                                "text",
                                "auto",
                                fields.no,
                                on_no_input,
                                on_no_auto,
                            )}
                            <div class="hint mb-2">
                                "Guess: " {move || Ymd::parse(&fields.date.get())
                                    .and_then(|date| finance_month_index(date.month))
                                    .map(|index| index.to_string())
                                    .unwrap_or_else(|| "—".to_owned())}
                            </div>
                            {form_input("Reference no.", "text", fields.reference, bind_field(fields.reference))}
                        </div>
                    </div>

                    <div class="card mt-3">
                        <div class="card-header py-2">
                            <strong>"Line item"</strong>
                        </div>
                        <div class="card-body">
                            {form_input("Item description", "text", fields.item, bind_field(fields.item))}
                            <div class="row">
                                <div class="col-sm-6">
                                    {form_input("HSN / SAC", "text", fields.hsn, bind_field(fields.hsn))}
                                </div>
                            </div>
                            <div class="row">
                                <div class="col-sm-6">
                                    {form_input("Hours", "number", fields.hours, bind_field(fields.hours))}
                                </div>
                                <div class="col-sm-6">
                                    {form_input("Rate per hour", "number", fields.rate, bind_field(fields.rate))}
                                </div>
                            </div>
                            <div class="hint mb-1">"Total:" <strong>{move || fmt_amount(total.get().unwrap_or(0.0))}</strong></div>
                        </div>
                    </div>
                </div>

                <div class="col-lg-7 invoice-col">
                    <InvoiceSheet cfg=cfg fields=fields total=total/>
                </div>
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Live invoice preview (mirrors the layout of the original template)
// ---------------------------------------------------------------------------

#[component]
fn InvoiceSheet(
    cfg: RwSignal<Config>,
    fields: InvoiceFields,
    total: Memo<Option<f64>>,
) -> impl IntoView {
    let full_no = full_invoice_no(cfg, fields);

    view! {
        <div class="invoice-sheet" id="invoice-sheet">
            <div class="card">
                <div class="card-header">
                    "Invoice No "
                    <strong>{move || full_no.get()}</strong>
                    <span class="float-right">
                        <strong>"Tax Invoice"</strong>
                        " For Recipient"
                    </span>
                </div>

                <div class="card-body">
                    <div class="row">
                        <div class="col mb-8"></div>
                        <div class="col mb-2">
                            <div class="row">
                                <div class="col-sm-3 font-weight-bold">"GSTIN"</div>
                                <div class="col text-muted">{move || cfg.get().seller_gstin.clone()}</div>
                            </div>
                            <div class="row">
                                <div class="col-sm-3 font-weight-bold">"State"</div>
                                <div class="col text-muted">{move || cfg.get().seller_state.clone()}</div>
                            </div>
                            <div class="row">
                                <div class="col-sm-3 font-weight-bold">"PAN"</div>
                                <div class="col text-muted">{move || cfg.get().seller_pan.clone()}</div>
                            </div>
                        </div>
                        <div class="col mb-2">
                            <div class="row">
                                <div class="col-sm-5 font-weight-bold">"Invoice Date"</div>
                                <div class="col text-muted">{move || fields.date.get()}</div>
                            </div>
                            <div class="row">
                                <div class="col-sm-5 font-weight-bold">"Invoice No"</div>
                                <div class="col text-muted">{move || full_no.get()}</div>
                            </div>
                            <div class="row">
                                <div class="col font-weight-bold">"Reference No."</div>
                                <div class="col text-muted">{move || fields.reference.get()}</div>
                            </div>
                        </div>
                    </div>
                </div>

                <hr/>

                <div class="card-body">
                    <div class="row mb-4">
                        <div class="col-sm-6">
                            <h6 class="mb-3">"From:"</h6>
                            <div><strong>{move || cfg.get().seller_name.clone()}</strong></div>
                            <div>{move || cfg.get().seller_address_1.clone()}</div>
                            <div>{move || cfg.get().seller_address_2.clone()}</div>
                            <div>{move || format!("PinCode: {}", cfg.get().seller_pincode)}</div>
                            <div>{move || format!("Phone: {}", cfg.get().seller_phone)}</div>
                        </div>

                        <div class="col-sm-6">
                            <h6 class="mb-3">"To:"</h6>
                            <div><strong>{move || cfg.get().buyer_name.clone()}</strong></div>
                            <div>{move || cfg.get().buyer_address_1.clone()}</div>
                            <div>{move || cfg.get().buyer_address_2.clone()}</div>
                            <div>{move || format!("Website: {}", cfg.get().buyer_website)}</div>
                        </div>
                    </div>

                    <div class="table-responsive-sm">
                        <table class="table table-active">
                            <tbody>
                                <tr>
                                    <th>
                                        <strong>"Place of Supply"</strong>
                                        " " {move || cfg.get().place_of_supply.clone()}
                                    </th>
                                    <th>
                                        <strong>"Due Date"</strong>
                                        " " {move || fields.due.get()}
                                    </th>
                                    <th>
                                        <strong>"Country of Supply"</strong>
                                        " " {move || cfg.get().country_of_supply.to_uppercase()}
                                    </th>
                                </tr>
                            </tbody>
                        </table>
                    </div>

                    <div class="table-responsive-sm">
                        <table class="table table-striped">
                            <thead>
                                <tr>
                                    <th class="center">"#"</th>
                                    <th>"Item"</th>
                                    <th>"HSN/SAC"</th>
                                    <th>"Quantity"</th>
                                    <th>"Rate/Item"</th>
                                    <th>"Discount"</th>
                                    <th>"Taxable Value"</th>
                                    <th>"IGST"</th>
                                    <th>"CESS"</th>
                                    <th class="right">"Total"</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr>
                                    <td class="center">"1"</td>
                                    <td class="left strong">{move || fields.item.get()}</td>
                                    <td class="left">{move || fields.hsn.get()}</td>
                                    <td class="right">{move || fields.hours.get()}</td>
                                    <td class="center">{move || fields.rate.get()}</td>
                                    <td class="right">"0.00"</td>
                                    <td class="right">{move || fmt_amount(total.get().unwrap_or(0.0))}</td>
                                    <td class="right">"0.00"</td>
                                    <td class="right">"0.00"</td>
                                    <td class="right">{move || fmt_amount(total.get().unwrap_or(0.0))}</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>

                    <div class="row">
                        <div class="col-lg-4 col-sm-5"></div>
                        <div class="col-lg-4 col-sm-5 ml-auto">
                            <table class="table table-clear">
                                <tbody>
                                    <tr>
                                        <td class="left"><strong>"Taxable Amount"</strong></td>
                                        <td class="right">{move || fmt_amount(total.get().unwrap_or(0.0))}</td>
                                    </tr>
                                    <tr>
                                        <td class="left"><strong>"Total Tax"</strong></td>
                                        <td class="right">"0.00"</td>
                                    </tr>
                                    <tr>
                                        <td class="left"><strong>"Total"</strong></td>
                                        <td class="right"><strong>{move || fmt_amount(total.get().unwrap_or(0.0))}</strong></td>
                                    </tr>
                                </tbody>
                            </table>
                        </div>
                    </div>

                    <br/>
                    <br/>
                    <div class="table-responsive-sm">
                        <h6 class="mb-3">
                            <strong>{move || cfg.get().declaration_text.clone()}</strong>
                        </h6>
                    </div>
                </div>
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Settings tab: everything configurable, auto-saved to localStorage
// ---------------------------------------------------------------------------

#[component]
fn SettingsWorkspace(cfg: RwSignal<Config>) -> impl IntoView {
    let on_reset = move |_| {
        cfg.set(Config::default());
        persist_cfg(cfg);
    };

    view! {
        <div class="row">
            <div class="col-lg-5 mb-3">
                <div class="card">
                    <div class="card-header py-2">
                        <strong>"Seller (From)"</strong>
                    </div>
                    <div class="card-body">
                        {text_input_group("Legal name", move || cfg.get().seller_name.clone(), bind_cfg(cfg, |c, v| c.seller_name = v))}
                        <div class="row">
                            <div class="col-sm-6">
                                {text_input_group("GSTIN", move || cfg.get().seller_gstin.clone(), bind_cfg(cfg, |c, v| c.seller_gstin = v))}
                            </div>
                            <div class="col-sm-6">
                                {text_input_group("PAN", move || cfg.get().seller_pan.clone(), bind_cfg(cfg, |c, v| c.seller_pan = v))}
                            </div>
                        </div>
                        {text_input_group("State (code-name)", move || cfg.get().seller_state.clone(), bind_cfg(cfg, |c, v| c.seller_state = v))}
                        {text_input_group("Address line 1", move || cfg.get().seller_address_1.clone(), bind_cfg(cfg, |c, v| c.seller_address_1 = v))}
                        {text_input_group("Address line 2", move || cfg.get().seller_address_2.clone(), bind_cfg(cfg, |c, v| c.seller_address_2 = v))}
                        <div class="row">
                            <div class="col-sm-6">
                                {text_input_group("Pin code", move || cfg.get().seller_pincode.clone(), bind_cfg(cfg, |c, v| c.seller_pincode = v))}
                            </div>
                            <div class="col-sm-6">
                                {text_input_group("Phone", move || cfg.get().seller_phone.clone(), bind_cfg(cfg, |c, v| c.seller_phone = v))}
                            </div>
                        </div>
                    </div>
                </div>

                <div class="card mt-3">
                    <div class="card-header py-2">
                        <strong>"Buyer (To)"</strong>
                    </div>
                    <div class="card-body">
                        {text_input_group("Name", move || cfg.get().buyer_name.clone(), bind_cfg(cfg, |c, v| c.buyer_name = v))}
                        <div class="row">
                            <div class="col-sm-6">
                                {text_input_group("Address line 1", move || cfg.get().buyer_address_1.clone(), bind_cfg(cfg, |c, v| c.buyer_address_1 = v))}
                            </div>
                            <div class="col-sm-6">
                                {text_input_group("Address line 2", move || cfg.get().buyer_address_2.clone(), bind_cfg(cfg, |c, v| c.buyer_address_2 = v))}
                            </div>
                        </div>
                        <div class="row">
                            <div class="col-sm-6">
                                {text_input_group("Website", move || cfg.get().buyer_website.clone(), bind_cfg(cfg, |c, v| c.buyer_website = v))}
                            </div>
                            <div class="col-sm-6">
                                {text_input_group("Place of supply", move || cfg.get().place_of_supply.clone(), bind_cfg(cfg, |c, v| c.place_of_supply = v))}
                            </div>
                        </div>
                        {text_input_group("Country of supply", move || cfg.get().country_of_supply.clone(), bind_cfg(cfg, |c, v| c.country_of_supply = v))}
                    </div>
                </div>
            </div>

            <div class="col-lg-5 mb-3">
                <div class="card">
                    <div class="card-header py-2">
                        <strong>"Invoice defaults"</strong>
                    </div>
                    <div class="card-body">
                        <div class="row">
                            <div class="col-sm-6">
                                {text_input_group("Invoice number prefix", move || cfg.get().invoice_prefix.clone(), bind_cfg(cfg, |c, v| c.invoice_prefix = v))}
                            </div>
                            <div class="col-sm-6">
                                {text_input_group("Default rate per hour", move || cfg.get().default_rate.clone(), bind_cfg(cfg, |c, v| c.default_rate = v))}
                            </div>
                        </div>
                        {text_input_group("Default item description", move || cfg.get().default_item.clone(), bind_cfg(cfg, |c, v| c.default_item = v))}
                        <div class="row">
                            <div class="col-sm-6">
                                {text_input_group("Default HSN / SAC", move || cfg.get().default_hsn.clone(), bind_cfg(cfg, |c, v| c.default_hsn = v))}
                            </div>
                        </div>
                        {textarea_group("Declaration text (bottom of invoice)", move || cfg.get().declaration_text.clone(), bind_cfg_textarea(cfg, |c, v| c.declaration_text = v))}
                    </div>
                </div>

                <div class="card mt-3">
                    <div class="card-body">
                        <button class="btn btn-outline-danger btn-sm" on:click=on_reset>
                            "Reset to placeholder defaults"
                        </button>
                        <div class="hint mt-2">
                            "Settings are stored in this browser (localStorage) and restored automatically the next time the page loads."
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}
