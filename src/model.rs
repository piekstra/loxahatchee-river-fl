//! Small normalized models for tenant info, service status, and payments.

use serde::Serialize;
use serde_json::Value;

/// Tenant/district configuration distilled from `/metadata` + `/capabilities`.
#[derive(Debug, Clone, Serialize)]
pub struct District {
    pub wipp_id: String,
    pub name: String,
    pub state: String,
    /// Services this district bills and which of them accept online payment.
    pub services: Vec<ServiceInfo>,
    /// Free-form "questions? contact …" message shown in the portal.
    pub contact_message: String,
    /// Maximum overpayment the tenant allows on a utility account.
    pub overpay_limit: f64,
    /// Bill types the portal is configured to accept (from `/capabilities`).
    pub accepts_utility_bill: bool,
    /// Names of enabled portal feature flags (from `/additional-metadata`).
    pub features: Vec<String>,
    /// Payment methods the tenant disallows, if any.
    pub disallowed_payment_methods: Vec<String>,
}

/// One service line in [`District::services`].
#[derive(Debug, Clone, Serialize)]
pub struct ServiceInfo {
    pub service: String,
    pub installed: bool,
    pub accepts_payment: bool,
    pub metered: bool,
}

impl District {
    /// Combine the `/metadata`, `/capabilities`, and `/additional-metadata`
    /// bodies into a [`District`]. `extra` may be `Value::Null` if unavailable.
    pub fn from_nodes(wipp_id: &str, meta: &Value, caps: &Value, extra: &Value) -> District {
        let svc = |label: &str, inst: &str, pay: &str, metered: &str| ServiceInfo {
            service: label.to_string(),
            installed: bool_at(meta, inst),
            accepts_payment: bool_at(meta, pay),
            metered: bool_at(meta, metered),
        };
        let services = vec![
            svc("Water", "wtrInstalled", "acceptWtrPayments", "wtrMetered"),
            svc("Sewer", "swrInstalled", "acceptSwrPayments", "swrMetered"),
            svc(
                "Electric",
                "elcInstalled",
                "acceptElcPayments",
                "elcMetered",
            ),
            svc("Other", "otrInstalled", "acceptOtrPayments", "otrMetered"),
        ]
        .into_iter()
        .filter(|s| s.installed)
        .collect();

        let mut features: Vec<String> = extra
            .get("featureFlags")
            .and_then(Value::as_object)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        features.sort();
        let disallowed_payment_methods = extra
            .get("disallowedPaymentMethods")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();

        District {
            wipp_id: wipp_id.to_string(),
            name: string_at(meta, "cityName"),
            state: string_at(meta, "state"),
            services,
            contact_message: string_at(meta, "contactMessage"),
            overpay_limit: meta
                .get("utilOverpayAmt")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            accepts_utility_bill: bool_at(caps, "utilBill"),
            features,
            disallowed_payment_methods,
        }
    }
}

/// One posted payment from the account's history.
#[derive(Debug, Clone, Serialize)]
pub struct Payment {
    pub transaction_id: String,
    pub amount: f64,
    pub payment_date: String,
    pub posted_time: String,
    pub method: String,
    pub account_type: String,
}

impl Payment {
    pub fn from_node(v: &Value) -> Payment {
        Payment {
            transaction_id: string_at(v, "transactionId"),
            amount: v.get("amt").and_then(Value::as_f64).unwrap_or(0.0),
            payment_date: string_at(v, "paymentDate"),
            posted_time: string_at(v, "postTime"),
            method: string_at(v, "paymentMethodCode"),
            account_type: string_at(v, "accountType"),
        }
    }
}

/// Per-service active/inactive status from `determineAccountStatus`.
#[derive(Debug, Clone, Serialize)]
pub struct AccountStatus {
    /// Raw one-letter codes keyed by service (`A` active, `N` none/inactive).
    pub overall: String,
    pub water: String,
    pub sewer: String,
    pub electric: String,
    pub other: String,
}

impl AccountStatus {
    pub fn from_node(v: &Value) -> AccountStatus {
        AccountStatus {
            overall: string_at(v, "accountStatus"),
            water: string_at(v, "wtrStatus"),
            sewer: string_at(v, "swrStatus"),
            electric: string_at(v, "elcStatus"),
            other: string_at(v, "otrStatus"),
        }
    }
}

/// Expand a one-letter service-status code to a word.
pub fn status_word(code: &str) -> &'static str {
    match code.trim() {
        "A" => "active",
        "N" | "" => "inactive",
        _ => "unknown",
    }
}

fn string_at(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn bool_at(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn district_keeps_only_installed_services() {
        let meta = json!({
            "cityName": "Loxahatchee River District", "state": "FL",
            "wtrInstalled": false, "swrInstalled": true, "elcInstalled": false, "otrInstalled": true,
            "acceptSwrPayments": true, "acceptOtrPayments": true,
            "swrMetered": false, "otrMetered": false,
            "utilOverpayAmt": 1000.0, "contactMessage": "call us"
        });
        let caps = json!({ "utilBill": true });
        let extra = json!({
            "featureFlags": { "RedactOwnerName": {}, "AllowBankruptPayment": {} },
            "disallowedPaymentMethods": []
        });
        let d = District::from_nodes("LOXA", &meta, &caps, &extra);
        assert_eq!(d.services.len(), 2);
        assert_eq!(d.services[0].service, "Sewer");
        assert!(d.accepts_utility_bill);
        assert_eq!(d.overpay_limit, 1000.0);
        assert_eq!(d.features, vec!["AllowBankruptPayment", "RedactOwnerName"]);
    }

    #[test]
    fn status_codes_expand() {
        assert_eq!(status_word("A"), "active");
        assert_eq!(status_word("N"), "inactive");
        assert_eq!(status_word(" "), "inactive");
    }
}
