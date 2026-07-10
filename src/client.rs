//! Thin, polite client over the Edmunds/SunGard "WIPP" utility-billing API that
//! powers the Loxahatchee River District payments portal
//! (`wipp.edmundsgovtech.cloud`, tenant id `LOXA`).
//!
//! All reads here are the same anonymous, guest-view calls the portal itself
//! makes before you log in — looking up an account by number to show its
//! balance and pay it. No credentials are involved.
//!
//! Two things gate the API and are handled here:
//! * an AWS WAF that rejects non-browser `User-Agent`s (we send a browser UA
//!   with a polite tool suffix), and
//! * the tenant selector header `X-Wipp-Id`.
//!
//! Some operations are **asynchronous** on the mainframe side: the request
//! returns `202 { requestId }` and you poll `GET /requests/{id}` until it flips
//! to `200` with the result. [`Wipp::await_request`] implements that loop.

use std::thread::sleep;
use std::time::Duration;

use serde_json::Value;

use crate::acct::AccountId;
use crate::error::AppError;

/// Production WIPP core API base (baked into the portal's own JS bundle).
pub const BASE: &str = "https://api.edmundsgovtech.cloud/wipp-core/v1";
/// Default tenant id — the Loxahatchee River District.
pub const DEFAULT_WIPP_ID: &str = "LOXA";
/// Public web origin (used to build portal deep links for `pay` / `open`).
pub const PORTAL_ORIGIN: &str = "https://wipp.edmundsgovtech.cloud";

/// A browser-shaped User-Agent with a polite tool suffix. The tenant's WAF
/// blocks obvious non-browser agents (a bare client string gets a 403), so we
/// present as a browser while still identifying the tool.
const UA: &str = concat!(
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 ",
    "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 ",
    "loxahatchee-river-fl/",
    env!("CARGO_PKG_VERSION"),
);

/// How long to keep polling an async `/requests/{id}` before giving up.
const POLL_BUDGET: Duration = Duration::from_secs(20);
/// Delay between poll attempts.
const POLL_INTERVAL: Duration = Duration::from_millis(700);

/// Client bound to a single tenant (`wipp_id`).
pub struct Wipp {
    client: reqwest::blocking::Client,
    wipp_id: String,
}

impl Wipp {
    pub fn new(wipp_id: String) -> Result<Self, AppError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(UA)
            .timeout(Duration::from_secs(25))
            .build()
            .map_err(|e| AppError::Other(format!("failed to build HTTP client: {e}")))?;
        Ok(Self { client, wipp_id })
    }

    pub fn wipp_id(&self) -> &str {
        &self.wipp_id
    }

    /// `GET {BASE}{path}` with the tenant header. `path` must be pre-encoded and
    /// begin with `/`. Returns `(status, parsed-json-or-null)`.
    fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<(u16, Value), AppError> {
        let url = format!("{BASE}{path}");
        let resp = self
            .client
            .get(&url)
            .header("X-Wipp-Id", &self.wipp_id)
            .header("Accept", "application/json, text/plain, */*")
            .query(query)
            .send()?;
        parse_response(resp)
    }

    /// `GET {BASE}{path}` with the tenant header and a bearer token, for
    /// authenticated (logged-in) endpoints.
    fn get_authed(&self, path: &str, bearer: &str) -> Result<(u16, Value), AppError> {
        let url = format!("{BASE}{path}");
        let resp = self
            .client
            .get(&url)
            .header("X-Wipp-Id", &self.wipp_id)
            .header("Accept", "application/json, text/plain, */*")
            .bearer_auth(bearer)
            .send()?;
        parse_response(resp)
    }

    /// `POST {BASE}{path}` with a JSON body, the tenant header, and an optional
    /// bearer token.
    fn post(
        &self,
        path: &str,
        body: &Value,
        bearer: Option<&str>,
    ) -> Result<(u16, Value), AppError> {
        let url = format!("{BASE}{path}");
        let mut req = self
            .client
            .post(&url)
            .header("X-Wipp-Id", &self.wipp_id)
            .header("Accept", "application/json, text/plain, */*")
            .json(body);
        if let Some(token) = bearer {
            req = req.bearer_auth(token);
        }
        parse_response(req.send()?)
    }

    /// Map a non-success status + body into the right [`AppError`], for the
    /// endpoints where a plain GET is expected to succeed directly.
    fn require_ok(&self, status: u16, body: Value, ctx: &str) -> Result<Value, AppError> {
        match status {
            200 => Ok(body),
            404 => Err(AppError::NotFound(
                body_message(&body).unwrap_or_else(|| ctx.to_string()),
            )),
            _ => Err(AppError::Network(format!(
                "HTTP {status} from {ctx}: {}",
                body_message(&body).unwrap_or_else(|| "no detail".into())
            ))),
        }
    }

    /// Tenant configuration: district name, which services are billed, overpay
    /// limits, contact message, etc. `GET /metadata/{wippId}`.
    pub fn metadata(&self) -> Result<Value, AppError> {
        let (s, b) = self.get(&format!("/metadata/{}", self.wipp_id), &[])?;
        self.require_ok(s, b, "metadata")
    }

    /// Which bill types this tenant accepts. `GET /capabilities/{wippId}`.
    pub fn capabilities(&self) -> Result<Value, AppError> {
        let (s, b) = self.get(&format!("/capabilities/{}", self.wipp_id), &[])?;
        self.require_ok(s, b, "capabilities")
    }

    /// Feature flags & third-party integrations for the tenant.
    /// `GET /wipp/additional-metadata/{wippId}`.
    pub fn additional_metadata(&self) -> Result<Value, AppError> {
        let (s, b) = self.get(&format!("/wipp/additional-metadata/{}", self.wipp_id), &[])?;
        self.require_ok(s, b, "additional-metadata")
    }

    /// Full utility account record (owner, service location, and per-service
    /// charge detail). `GET /wippUtil/{encodedAccountId}`.
    pub fn utility_account(&self, id: &AccountId) -> Result<Value, AppError> {
        let (s, b) = self.get(&format!("/wippUtil/{}", id.encoded()), &[])?;
        match s {
            200 => Ok(b),
            401 | 404 => Err(AppError::NotFound(format!(
                "no utility account {}",
                id.dashed()
            ))),
            _ => Err(AppError::Network(format!(
                "HTTP {s} looking up account {}: {}",
                id.dashed(),
                body_message(&b).unwrap_or_else(|| "no detail".into())
            ))),
        }
    }

    /// Per-service active/inactive status. This is an **async** mainframe call:
    /// submit, then poll. `GET /wippUtil/{id}/determineAccountStatus` → poll.
    pub fn account_status(&self, id: &AccountId) -> Result<Value, AppError> {
        let (s, b) = self.get(
            &format!("/wippUtil/{}/determineAccountStatus", id.encoded()),
            &[],
        )?;
        self.await_request(s, b, "account status")
    }

    /// Payment history since `after` (an ISO `YYYY-MM-DD` date).
    /// `GET /billingAccounts/wippUtil/{id}/payments?paymentsAfterDate=…`.
    pub fn payment_history(&self, id: &AccountId, after: &str) -> Result<Value, AppError> {
        let (s, b) = self.get(
            &format!("/billingAccounts/wippUtil/{}/payments", id.encoded()),
            &[("paymentsAfterDate", after)],
        )?;
        self.require_ok(s, b, "payment history")
    }

    /// Drive an async request to completion. The initial call either already
    /// returned `200` with the result, or `202 { requestId }` that we poll on
    /// `GET /requests/{id}` (also `202` while processing, `200` when done).
    fn await_request(&self, status: u16, body: Value, ctx: &str) -> Result<Value, AppError> {
        if status == 200 {
            return Ok(body);
        }
        if status != 202 {
            return Err(AppError::Network(format!(
                "HTTP {status} starting {ctx}: {}",
                body_message(&body).unwrap_or_else(|| "no detail".into())
            )));
        }
        let request_id = body
            .get("requestId")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Other(format!("{ctx}: server did not return a requestId")))?
            .to_string();

        let deadline = std::time::Instant::now() + POLL_BUDGET;
        loop {
            let (s, b) = self.get(&format!("/requests/{request_id}"), &[])?;
            match s {
                200 => return Ok(b),
                202 => {
                    if std::time::Instant::now() >= deadline {
                        return Err(AppError::Timeout(format!(
                            "{ctx} was still processing after {}s",
                            POLL_BUDGET.as_secs()
                        )));
                    }
                    sleep(POLL_INTERVAL);
                }
                other => {
                    return Err(AppError::Network(format!(
                        "HTTP {other} polling {ctx}: {}",
                        body_message(&b).unwrap_or_else(|| "no detail".into())
                    )))
                }
            }
        }
    }

    /// Portal deep link to the guest account view (where "Pay Now" lives).
    pub fn account_view_url(&self, id: &AccountId) -> String {
        format!(
            "{PORTAL_ORIGIN}/view/wippUtil/{}?wippId={}",
            id.dashed(),
            self.wipp_id
        )
    }

    // --- Authenticated (logged-in) endpoints --------------------------------
    //
    // The portal authenticates against AWS Cognito. `POST /auth` exchanges
    // email + password for a token set; `POST /auth/refreshToken` exchanges the
    // long-lived refresh token for a fresh access token. Both answer with a
    // `{ status, data, message }` envelope. These are reverse-engineered from
    // the portal SPA and exercised with a real login.

    /// Exchange email + password for a token set. `POST /auth`.
    pub fn authenticate(&self, email: &str, password: &str) -> Result<TokenSet, AppError> {
        let body = serde_json::json!({ "email": email, "password": password });
        let (status, resp) = self.post("/auth", &body, None)?;
        Self::parse_auth_envelope(status, resp)
    }

    /// Exchange a refresh token for a fresh token set. `POST /auth/refreshToken`.
    pub fn refresh(&self, email: &str, refresh_token: &str) -> Result<TokenSet, AppError> {
        let body = serde_json::json!({ "email": email, "refreshToken": refresh_token });
        let (status, resp) = self.post("/auth/refreshToken", &body, None)?;
        Self::parse_auth_envelope(status, resp)
    }

    /// The logged-in user's profile. `GET /accounts/cognitoUsers`.
    pub fn profile(&self, bearer: &str) -> Result<Value, AppError> {
        let (s, b) = self.get_authed("/accounts/cognitoUsers", bearer)?;
        self.require_authed(s, b, "profile")
    }

    /// Utility accounts linked to the logged-in user. `GET /accounts/billingAccounts`.
    pub fn billing_accounts(&self, bearer: &str) -> Result<Value, AppError> {
        let (s, b) = self.get_authed("/accounts/billingAccounts", bearer)?;
        self.require_authed(s, b, "billing accounts")
    }

    /// Saved scheduled payments. `GET /payments/schedules`.
    pub fn payment_schedules(&self, bearer: &str) -> Result<Value, AppError> {
        let (s, b) = self.get_authed("/payments/schedules", bearer)?;
        self.require_authed(s, b, "scheduled payments")
    }

    /// Saved wallet accounts / payment methods. `GET /wallet/Accounts`.
    pub fn wallet(&self, bearer: &str) -> Result<Value, AppError> {
        let (s, b) = self.get_authed("/wallet/Accounts", bearer)?;
        self.require_authed(s, b, "wallet")
    }

    /// Like [`Wipp::require_ok`] but maps `401`/`403` to an auth error so callers
    /// can prompt the user to log in again.
    fn require_authed(&self, status: u16, body: Value, ctx: &str) -> Result<Value, AppError> {
        match status {
            200 => Ok(body),
            401 | 403 => Err(AppError::Auth(format!(
                "session expired or unauthorized for {ctx} — run `lrfl login` again"
            ))),
            404 => Err(AppError::NotFound(
                body_message(&body).unwrap_or_else(|| ctx.to_string()),
            )),
            _ => Err(AppError::Network(format!(
                "HTTP {status} from {ctx}: {}",
                body_message(&body).unwrap_or_else(|| "no detail".into())
            ))),
        }
    }

    /// Parse a Cognito `{ status, data, message }` auth envelope into a
    /// [`TokenSet`], mapping failures (incl. the API's habit of surfacing bad
    /// credentials as `500`) to a clear [`AppError::Auth`].
    fn parse_auth_envelope(status: u16, resp: Value) -> Result<TokenSet, AppError> {
        let msg = resp
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if msg == "New_Password_Required" {
            return Err(AppError::Auth(
                "this account must set a new password in the portal before it can be used here"
                    .into(),
            ));
        }
        let ok = resp
            .get("status")
            .and_then(Value::as_str)
            .map(|s| s.eq_ignore_ascii_case("SUCCESS"))
            .unwrap_or(false);
        let data = resp.get("data");
        if status == 200 && ok {
            if let Some(tokens) = data.and_then(TokenSet::from_node) {
                return Ok(tokens);
            }
        }
        // Anything else is an auth failure. The Cognito wrapper commonly returns
        // a bare `(500) …` for wrong or unknown credentials, so add a hint rather
        // than let it read like a server bug.
        let detail = if !msg.is_empty() {
            msg
        } else {
            body_message(&resp).unwrap_or_else(|| format!("HTTP {status}"))
        };
        let hint = if status == 500 || status == 401 || status == 403 {
            " — double-check your email and password"
        } else {
            ""
        };
        Err(AppError::Auth(format!("login failed: {detail}{hint}")))
    }
}

/// A Cognito token set. The refresh token is the long-lived credential we store;
/// the access token is short-lived and used only for the current invocation.
#[derive(Debug, Clone)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: String,
}

impl TokenSet {
    /// Parse from the `data` object of an auth envelope. Tolerates camelCase or
    /// snake_case keys.
    fn from_node(v: &Value) -> Option<TokenSet> {
        let pick = |keys: &[&str]| -> Option<String> {
            keys.iter()
                .find_map(|k| v.get(*k).and_then(Value::as_str))
                .map(str::to_string)
        };
        let access_token = pick(&["accessToken", "access_token", "idToken", "id_token"])?;
        // A refresh may not re-issue a refresh token; the caller keeps the old one.
        let refresh_token = pick(&["refreshToken", "refresh_token"]).unwrap_or_default();
        Some(TokenSet {
            access_token,
            refresh_token,
        })
    }
}

/// Read a blocking response into `(status, json-or-bare-string)`, treating 429
/// as a dedicated rate-limit error.
fn parse_response(resp: reqwest::blocking::Response) -> Result<(u16, Value), AppError> {
    let status = resp.status().as_u16();
    if status == 429 {
        return Err(AppError::RateLimited);
    }
    let text = resp.text()?;
    // The API returns JSON on success and a bare `(NNN) message` string on some
    // errors; tolerate both so callers get a useful message.
    let value = serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));
    Ok((status, value))
}

/// Pull a human message out of an error body, whether it arrived as a JSON
/// `{message}` / `{error}` object or as the API's bare `(NNN) text` string.
fn body_message(body: &Value) -> Option<String> {
    if let Some(s) = body.as_str() {
        let s = s.trim();
        return (!s.is_empty()).then(|| s.to_string());
    }
    body.get("message")
        .or_else(|| body.get("error"))
        .and_then(Value::as_str)
        .map(str::to_string)
}
