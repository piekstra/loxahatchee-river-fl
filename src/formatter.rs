//! Human-readable and `--json` rendering for every command.
//!
//! The CLI faithfully shows whatever the district's portal returns for an
//! account — owner name and mailing address included. It doesn't impose privacy
//! the provider itself doesn't.

use serde_json::{json, Value};

use crate::account::Account;
use crate::model::{status_word, AccountMatch, AccountStatus, District, LinkedAccount, Payment};

fn money(x: f64) -> String {
    format!("${x:.2}")
}

fn or_dash(s: &str) -> &str {
    if s.trim().is_empty() {
        "—"
    } else {
        s
    }
}

fn print_json(v: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(v).expect("serialize json")
    );
}

pub fn print_account(a: &Account, json: bool) {
    if json {
        print_json(&serde_json::to_value(a).expect("serialize account"));
        return;
    }
    println!("Account {}", a.id);
    println!("  Balance due:   {}", money(a.balance_due));
    if !a.service_location.is_empty() {
        println!("  Service loc:   {}", a.service_location);
    }
    // Property location is often identical to the service address; only show it
    // when it differs, to avoid a redundant line.
    let prop = a.property_location.trim();
    if !prop.is_empty() && prop != a.service_location.trim() {
        println!("  Property loc:  {prop}");
    }
    println!("  Bill to:       {}", or_dash(&a.bill_to_name));
    println!("  Owner:         {}", or_dash(&a.owner.name));
    let addr = a.owner.address_line();
    if !addr.is_empty() {
        println!("  Mailing:       {addr}");
    }
    if !a.interest_date.is_empty() {
        println!("  Interest thru: {}", a.interest_date);
    }
    println!();
    println!("  Services:");
    for c in &a.charges {
        let due = if c.current_due_date.trim().is_empty() {
            String::new()
        } else {
            format!("   due {}", c.current_due_date)
        };
        println!("    {} — {}{}", c.service, money(c.amount_due), due);
        println!(
            "      principal {}   interest {}",
            money(c.total_principal),
            money(c.total_interest)
        );
        if c.future_principal.abs() > f64::EPSILON {
            println!("      not-yet-due principal {}", money(c.future_principal));
        }
        let mut ytd = format!("      billed YTD {}", money(c.billed_ytd));
        if !c.last_paid_date.trim().is_empty() {
            ytd.push_str(&format!("   last paid {}", c.last_paid_date));
        }
        println!("{ytd}");
        if !c.current_period_start.is_empty() || !c.current_period_end.is_empty() {
            println!(
                "      period {} – {}",
                or_dash(&c.current_period_start),
                or_dash(&c.current_period_end)
            );
        }
        if let Some(r) = c.current_reading {
            let mut m = format!("      reading {r}");
            if let Some(u) = c.current_usage {
                m.push_str(&format!("   usage {u}"));
            }
            if !c.current_reading_date.is_empty() {
                m.push_str(&format!("   ({})", c.current_reading_date));
            }
            println!("{m}");
        }
        if c.discount_amount.abs() > f64::EPSILON {
            let by = if c.discount_date.is_empty() {
                String::new()
            } else {
                format!(" by {}", c.discount_date)
            };
            println!("      early-pay discount {}{by}", money(c.discount_amount));
        }
    }
}

/// Address-search results: matched accounts with their location and owner.
pub fn print_search(query: &str, matches: &[AccountMatch], truncated: bool, json: bool) {
    if json {
        print_json(&json!({
            "query": query,
            "count": matches.len(),
            "truncated": truncated,
            "matches": matches,
        }));
        return;
    }
    if matches.is_empty() {
        println!("No accounts match {query:?}.");
        return;
    }
    println!(
        "{} match{} for {query:?}:",
        matches.len(),
        if matches.len() == 1 { "" } else { "es" }
    );
    for m in matches {
        // Balance column only appears with --extended (otherwise `None`).
        let bal = match m.balance_due {
            Some(b) => format!("{:>9}  ", money(b)),
            None => String::new(),
        };
        let who = if m.owner_name.is_empty() {
            String::new()
        } else {
            format!("   {}", m.owner_name)
        };
        println!(
            "  {:<11} {bal}{}{who}",
            m.account_id,
            or_dash(&m.property_location)
        );
    }
    if truncated {
        println!("  … page full — raise --limit to see more.");
    }
}

/// A combined overview: balance, per-service status, and the last payment.
pub fn print_summary(
    a: &Account,
    status: Option<&AccountStatus>,
    last_payment: Option<&Payment>,
    json: bool,
) {
    if json {
        let services: Vec<Value> = a
            .charges
            .iter()
            .map(|c| {
                let code = status.map(|s| s.code_for(&c.service)).unwrap_or("");
                json!({
                    "service": c.service,
                    "amount_due": c.amount_due,
                    "due_date": c.current_due_date,
                    "status": status.map(|_| status_word(code)),
                })
            })
            .collect();
        let mut out = json!({
            "account": a.id,
            "balance_due": a.balance_due,
            "services": services,
            "last_payment": last_payment.map(|p| serde_json::to_value(p).unwrap()),
        });
        out["owner"] = serde_json::to_value(&a.owner).unwrap_or(Value::Null);
        print_json(&out);
        return;
    }

    println!("Account {}", a.id);
    if !a.owner.name.is_empty() {
        println!("  {}", a.owner.name);
    }
    println!("  Balance due:  {}", money(a.balance_due));
    println!();
    for c in &a.charges {
        let status_tag = status
            .map(|s| format!("   [{}]", status_word(s.code_for(&c.service))))
            .unwrap_or_default();
        println!(
            "  {:<10} {:>10}   due {}{status_tag}",
            c.service,
            money(c.amount_due),
            or_dash(&c.current_due_date)
        );
    }
    match last_payment {
        Some(p) => println!(
            "\n  Last payment: {} on {} ({})",
            money(p.amount),
            or_dash(&p.payment_date),
            or_dash(&p.method)
        ),
        None => println!("\n  Last payment: none on record"),
    }
}

pub fn print_balance(a: &Account, json: bool) {
    if json {
        print_json(&json!({
            "account": a.id,
            "balance_due": a.balance_due,
            "services": a.charges.iter().map(|c| json!({
                "service": c.service,
                "amount_due": c.amount_due,
                "due_date": c.current_due_date,
            })).collect::<Vec<_>>(),
        }));
        return;
    }
    println!("Account {}", a.id);
    for c in &a.charges {
        println!(
            "  {:<10} {:>10}   due {}",
            c.service,
            money(c.amount_due),
            or_dash(&c.current_due_date)
        );
    }
    println!("  {:<10} {:>10}", "TOTAL", money(a.balance_due));
}

pub fn print_charges(a: &Account, json: bool) {
    if json {
        print_json(&json!({ "account": a.id, "charges": a.charges }));
        return;
    }
    println!("Account {} — charge detail\n", a.id);
    for c in &a.charges {
        println!("{}", c.service);
        println!("  Amount due:      {}", money(c.amount_due));
        println!("    principal:     {}", money(c.total_principal));
        println!("    interest:      {}", money(c.total_interest));
        if c.future_principal.abs() > f64::EPSILON {
            println!("    future (not due): {}", money(c.future_principal));
        }
        println!("  Due date:        {}", or_dash(&c.current_due_date));
        println!("  Last paid:       {}", or_dash(&c.last_paid_date));
        println!("  Billed YTD:      {}", money(c.billed_ytd));
        if c.discount_amount.abs() > f64::EPSILON {
            println!(
                "  Early-pay disc:  {} if paid by {}",
                money(c.discount_amount),
                or_dash(&c.discount_date)
            );
        }
        if !c.current_period_start.is_empty() || !c.current_period_end.is_empty() {
            println!(
                "  Current period:  {} → {}",
                or_dash(&c.current_period_start),
                or_dash(&c.current_period_end)
            );
        }
        if let Some(r) = c.current_reading {
            let usage = c
                .current_usage
                .map(|u| format!(", usage {u}"))
                .unwrap_or_default();
            println!(
                "  Meter reading:   {r} on {}{usage}",
                or_dash(&c.current_reading_date)
            );
        }
        println!();
    }
}

pub fn print_status(account_id: &str, s: &AccountStatus, json: bool) {
    if json {
        let mut v = serde_json::to_value(s).expect("serialize status");
        if let Some(o) = v.as_object_mut() {
            o.insert("account".into(), json!(account_id));
        }
        print_json(&v);
        return;
    }
    println!("Account {account_id} — service status");
    let rows = [
        ("Overall", &s.overall),
        ("Water", &s.water),
        ("Sewer", &s.sewer),
        ("Electric", &s.electric),
        ("Other", &s.other),
    ];
    for (label, code) in rows {
        // Skip services the district doesn't track (blank + inactive).
        if label != "Overall" && code.trim().is_empty() {
            continue;
        }
        println!("  {:<9} {} ({})", label, status_word(code), or_dash(code));
    }
}

pub fn print_history(account_id: &str, payments: &[Payment], json: bool) {
    if json {
        print_json(&json!({ "account": account_id, "payments": payments }));
        return;
    }
    if payments.is_empty() {
        println!("Account {account_id} — no payments in the selected window");
        return;
    }
    println!("Account {account_id} — {} payment(s)\n", payments.len());
    for p in payments {
        println!(
            "  {:<12} {:>10}   {}   #{}",
            or_dash(&p.payment_date),
            money(p.amount),
            or_dash(&p.method),
            p.transaction_id
        );
    }
    let total: f64 = payments.iter().map(|p| p.amount).sum();
    println!("\n  {} across {} payment(s)", money(total), payments.len());
}

pub fn print_district(d: &District, json: bool) {
    if json {
        print_json(&serde_json::to_value(d).expect("serialize district"));
        return;
    }
    println!("{} ({}, {})", d.name, d.wipp_id, d.state);
    println!(
        "  Accepts utility bill payments: {}",
        yes_no(d.accepts_utility_bill)
    );
    if d.overpay_limit > 0.0 {
        println!(
            "  Max overpayment:               {}",
            money(d.overpay_limit)
        );
    }
    println!("  Billed services:");
    for s in &d.services {
        let mut tags = Vec::new();
        if s.accepts_payment {
            tags.push("payable online");
        }
        if s.metered {
            tags.push("metered");
        }
        let suffix = if tags.is_empty() {
            String::new()
        } else {
            format!("  [{}]", tags.join(", "))
        };
        println!("    {}{suffix}", s.service);
    }
    if !d.disallowed_payment_methods.is_empty() {
        println!(
            "  Disallowed methods: {}",
            d.disallowed_payment_methods.join(", ")
        );
    }
    if !d.features.is_empty() {
        println!("  Feature flags: {}", d.features.join(", "));
    }
    if !d.contact_message.is_empty() {
        println!("  Contact: {}", d.contact_message);
    }
}

pub fn print_pay(account_id: &str, amount: f64, url: &str, opened: bool, json: bool) {
    if json {
        print_json(&json!({
            "account": account_id,
            "amount_due": amount,
            "payment_url": url,
            "opened": opened,
        }));
        return;
    }
    println!("Account {account_id}");
    println!("  Amount due: {}", money(amount));
    if opened {
        println!("  Opened the district's secure Pay Now page in your browser.");
    } else {
        println!("  Pay securely at:\n    {url}");
        println!("  (or re-run with --open to launch it)");
    }
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

/// Who you're logged in as. `claims` are the decoded `id_token` fields (FIS
/// issues `FirstName`, `sub` = login, `UID`, `UserType`, `LastLoginDate`); when
/// `None`, we're not logged in.
pub fn print_whoami(login: Option<&str>, claims: Option<&Value>, json: bool) {
    if json {
        print_json(&json!({
            "login": login,
            "logged_in": claims.is_some(),
            "claims": claims.cloned().unwrap_or(Value::Null),
        }));
        return;
    }
    let Some(claims) = claims else {
        match login {
            Some(l) => println!("not logged in ({l} has no stored session)"),
            None => println!("not logged in — run `lrfl login`"),
        }
        return;
    };
    let field = |k: &str| {
        claims
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let name = field("FirstName");
    let who = login.unwrap_or("").to_string();
    println!(
        "logged in as {}",
        if who.is_empty() { field("sub") } else { who }
    );
    if !name.is_empty() {
        println!("  name:      {name}");
    }
    let uid = field("UID");
    if !uid.is_empty() {
        println!("  user id:   {uid}");
    }
    let user_type = field("UserType");
    if !user_type.is_empty() {
        println!("  user type: {user_type}");
    }
}

/// Utility accounts linked to the login (from `--json` the raw normalized list).
/// Balances are shown only when they were requested (`accounts --balances`).
pub fn print_accounts(accounts: &[LinkedAccount], json: bool) {
    if json {
        print_json(&serde_json::to_value(accounts).expect("serialize accounts"));
        return;
    }
    if accounts.is_empty() {
        println!("no accounts linked to this login");
        return;
    }
    println!("Linked accounts ({})\n", accounts.len());
    for (i, a) in accounts.iter().enumerate() {
        let type_label = if a.account_type.is_empty() {
            String::new()
        } else {
            format!("  ({})", a.account_type)
        };
        let bal = a
            .balance_due
            .map(|b| format!("   {} due", money(b)))
            .unwrap_or_default();
        println!("  {}. {}{type_label}{bal}", i + 1, a.account_id);
    }
    let total: f64 = accounts.iter().filter_map(|a| a.balance_due).sum();
    if accounts.iter().any(|a| a.balance_due.is_some()) {
        println!(
            "\n  {} due across {} account(s)",
            money(total),
            accounts.len()
        );
    }
}
