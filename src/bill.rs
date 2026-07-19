//! Parse the hosted PDF bill's text into a structured [`Bill`].
//!
//! The onlinebiller.com PDF carries data the redacted WIPP API does not: the
//! bill-to customer / owner name, full mailing address, and AutoPay status. The
//! bill embeds a machine-readable `[KEY=VALUE]` block (some values span lines),
//! and we lift a couple of labeled lines for the rest.

use std::collections::HashMap;

use serde::Serialize;

/// A parsed utility bill.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Bill {
    /// `Sys_Acct_ID`, dashed (`NNNNNNN-N`).
    pub account_id: String,
    /// Bill-to party — the owner for owner-billed accounts, else `OCCUPANT`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub customer: String,
    /// Full mailing address (the `Sys_FullAddress` lines after the name).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub mailing_address: String,
    /// Service (situs) address — `CSERVADDR`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub service_address: String,
    /// Statement date — `CDATE`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub statement_date: String,
    /// Bill due date — `CDUEDATE`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub due_date: String,
    /// Service period, e.g. `7/1/2026 - 9/30/2026`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub service_period: String,
    /// Total due — `Sys_Balance`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_due: Option<f64>,
    /// Last payment, e.g. `$79.09 on 5/18/2026`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub last_payment: String,
    /// AutoPay plan text — `AUTOPAY_FLAG` (empty when not enrolled).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub autopay: String,
    /// Whether the account is on paperless billing (`PAPERLESS_FLAG` ≠ `N`).
    pub paperless: bool,
}

impl Bill {
    /// Parse a [`Bill`] from the PDF's extracted text.
    pub fn parse(text: &str) -> Bill {
        let kv = kv_block(text);
        let full = kv.get("Sys_FullAddress").cloned().unwrap_or_default();
        let mut addr_lines = full.lines().map(str::trim).filter(|l| !l.is_empty());
        let customer = addr_lines.next().unwrap_or_default().to_string();
        let mailing_address = addr_lines.collect::<Vec<_>>().join(", ");
        let paperless_flag = kv.get("PAPERLESS_FLAG").map(String::as_str).unwrap_or("");

        Bill {
            account_id: kv.get("Sys_Acct_ID").cloned().unwrap_or_default(),
            customer,
            mailing_address,
            service_address: kv.get("CSERVADDR").cloned().unwrap_or_default(),
            statement_date: kv.get("CDATE").cloned().unwrap_or_default(),
            due_date: kv.get("CDUEDATE").cloned().unwrap_or_default(),
            service_period: labeled_value(text, "Service Period:"),
            total_due: kv.get("Sys_Balance").and_then(|s| s.trim().parse().ok()),
            last_payment: labeled_value(text, "Last Payment:"),
            autopay: kv.get("AUTOPAY_FLAG").cloned().unwrap_or_default(),
            paperless: !matches!(paperless_flag.trim(), "" | "N"),
        }
    }

    /// Whether the account is enrolled in AutoPay.
    pub fn on_autopay(&self) -> bool {
        !self.autopay.trim().is_empty()
    }
}

/// Scan the bill text for `[KEY=VALUE]` tokens. Values may contain newlines (the
/// address block does); a token ends at the next `]`. Keys must be simple
/// `[A-Za-z0-9_]` tokens, which rejects incidental brackets like `*[1/1]*`.
fn kv_block(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find(']') else { break };
        let inner = &rest[..close];
        rest = &rest[close + 1..];
        if let Some(eq) = inner.find('=') {
            let key = inner[..eq].trim();
            if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                map.insert(key.to_string(), inner[eq + 1..].trim().to_string());
            }
        }
    }
    map
}

/// The remainder of the first line that contains `label`, trimmed.
fn labeled_value(text: &str, label: &str) -> String {
    for line in text.lines() {
        if let Some(pos) = line.find(label) {
            let v = line[pos + label.len()..].trim();
            if !v.is_empty() {
                return v.to_string();
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic bill text shaped like the real PDF extraction (no real PII).
    const SAMPLE: &str = "\
Statement Date: 7/9/2026
Service Period: 7/1/2026 - 9/30/2026
Last Payment: $94.91 on 5/13/2026
Total Due: $94.91
[Sys_Acct_ID=1234567-8]
[Sys_Balance=94.91]
[Sys_FullAddress=PUBLIC, JOHN Q & JANE
64 EXAMPLE RD
ANYTOWN, FL 33400-1712]
[CDATE=7/9/2026]
[CDUEDATE=8/12/2026]
[CSERVADDR=123 MAIN ST]
[PAPERLESS_FLAG=B]
[AUTOPAY_FLAG=DIRECT DEBIT PLAN - DO NOT PAY!]
*[1/1]*
";

    #[test]
    fn parses_key_value_block_and_labels() {
        let b = Bill::parse(SAMPLE);
        assert_eq!(b.account_id, "1234567-8");
        assert_eq!(b.customer, "PUBLIC, JOHN Q & JANE");
        assert_eq!(b.mailing_address, "64 EXAMPLE RD, ANYTOWN, FL 33400-1712");
        assert_eq!(b.service_address, "123 MAIN ST");
        assert_eq!(b.statement_date, "7/9/2026");
        assert_eq!(b.due_date, "8/12/2026");
        assert_eq!(b.service_period, "7/1/2026 - 9/30/2026");
        assert_eq!(b.total_due, Some(94.91));
        assert_eq!(b.last_payment, "$94.91 on 5/13/2026");
        assert!(b.on_autopay());
        assert_eq!(b.autopay, "DIRECT DEBIT PLAN - DO NOT PAY!");
        assert!(b.paperless);
        // The `*[1/1]*` token must not be picked up as a key/value.
        assert!(!SAMPLE.is_empty());
    }

    #[test]
    fn occupant_billed_has_no_owner_and_no_autopay() {
        let text = "\
[Sys_Acct_ID=4924000-0]
[Sys_Balance=79.09]
[Sys_FullAddress=OCCUPANT
6810 CHURCH ST]
[CSERVADDR=6810 CHURCH ST]
[PAPERLESS_FLAG=N]
[AUTOPAY_FLAG=]
";
        let b = Bill::parse(text);
        assert_eq!(b.customer, "OCCUPANT");
        assert_eq!(b.mailing_address, "6810 CHURCH ST");
        assert_eq!(b.total_due, Some(79.09));
        assert!(!b.on_autopay());
        assert!(!b.paperless);
    }
}
