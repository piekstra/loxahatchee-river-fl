# CLAUDE.md — loxahatchee-river-fl

Guidance for AI agents working in this repo.

## What this is

A Rust CLI (`lrfl`) over the **WIPP** (Edmunds GovTech / SunGard) utility-billing
API that powers the Loxahatchee River District payment portal (tenant `LOXA`).
Read commands are anonymous **guest-view** lookups by account number — no
credentials, no stored secrets. `lrfl pay` computes what's owed and hands off to
the portal's PCI-compliant Pay Now page rather than touching card data.

## Build / test / run

```sh
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all

./target/release/lrfl balance 1234567-0
./target/release/lrfl district --json
```

## Layout

- `src/cli.rs` — clap arg/command definitions.
- `src/client.rs` — the `Wipp` HTTP client: tenant header, browser UA, and the
  async submit-then-poll `/requests/{id}` pattern.
- `src/acct.rs` — utility account-id parsing (`NNNNNNN-N` ↔ padded API key). Tested.
- `src/account.rs` — the normalized `Account`/`ServiceCharge` model, the
  amount-due math, and defensive JSON parsing. Tested.
- `src/model.rs` — `District`, `Payment`, `AccountStatus` models. Tested.
- `src/config.rs` — the saved default-account file (not a secret).
- `src/output.rs` — human vs `--json` rendering, incl. owner redaction.
- `src/main.rs` — wiring, account resolution, date helpers, browser opener.
- `docs/wipp-api.md` — the discovered API (endpoints, shapes, gotchas). No PII.

## When chaining as an agent

- Use `--json`. stdout is pure JSON; stderr is diagnostics.
- Respect exit codes (see README): `2` usage, `4` not found, `5` network,
  `6` rate-limited, `7` async timeout.
- The field you usually want is `.balance_due` (or `.payments[].amount`).

## Gotchas & rules

- **No secrets, no PII in the repo.** Not in code, tests, fixtures, or commits.
  Tests use synthetic account data (`1234567-8`). CI-style secret scanning runs
  via `.githooks/pre-commit` (gitleaks). The one real account number that appears
  is only ever typed at runtime by the user — never bake it in.
- **Owner name/address is sensitive** (account numbers are enumerable). It's
  redacted in output unless `--show-owner`. Keep that default.
- The API's WAF blocks non-browser User-Agents — `client.rs` must send a
  browser-shaped UA. If reads start 403'ing, that's the first thing to check.
- Responses **drift** and some errors arrive as a bare `(NNN) message` string;
  parsing is best-effort. If a field goes missing, fix the path and add a test —
  don't make it required.
- Be polite: this is a public portal at human scale. No aggressive looping.
- Card payments intentionally go through the portal (reCAPTCHA + gateway). Don't
  add code that posts card data.
