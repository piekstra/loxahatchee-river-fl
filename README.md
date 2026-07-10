# loxahatchee-river-fl

Check your **Loxahatchee River District** utility account — balance, charges,
service status, and payment history — from the command line, and jump straight
to the district's secure page to pay.

It's a thin, polite client over the same **WIPP** (Edmunds GovTech / SunGard)
API that the district's payment portal at
[`wipp.edmundsgovtech.cloud`](https://wipp.edmundsgovtech.cloud/home?wippId=LOXA)
uses. Every read is an anonymous **guest-view** lookup by account number — the
same call the portal makes before you log in — so there's **no account,
password, or API key** to configure.

Built to be **human- and agent-friendly**: every command has a `--json` mode and
stable exit codes, so a person or a script can chain it into a pipeline.

> The crate/repo is `loxahatchee-river-fl`; the binary is **`lrfl`**.

## Install

```sh
cargo build --release        # binary at ./target/release/lrfl
# or
cargo install --path .       # installs the `lrfl` binary onto your PATH
```

Requires a recent stable Rust toolchain.

## Quick start

```sh
# Remember your account so later commands need no argument:
lrfl config set-account 1234567-0

lrfl balance                 # what do I owe?
lrfl charges                 # per-service detail, meter readings, usage
lrfl history --years 2       # recent payments
lrfl status                  # which services are active
lrfl pay --open              # compute the amount and open the Pay Now page
```

You can always pass an account explicitly (`lrfl balance 1234567-0`), or set
`$LRFL_ACCOUNT` instead of saving a default.

## Commands

| Command | What it shows |
|---------|---------------|
| `lrfl balance [ACCT]` | Amount due, per service and total |
| `lrfl account [ACCT]` | Full record: balance, service location, owner (hidden by default) |
| `lrfl charges [ACCT]` | Principal/interest, due dates, billed-YTD, meter readings & usage |
| `lrfl status [ACCT]` | Each service's active/inactive status |
| `lrfl history [ACCT]` | Posted payments (`--since YYYY-MM-DD` or `--years N`) |
| `lrfl pay [ACCT]` | Compute the amount due and hand off to the portal's Pay Now page (`--open`) |
| `lrfl open [ACCT]` | Open the account's portal page in your browser |
| `lrfl district` | District info: billed services, payment options, contact |
| `lrfl config …` | `set-account`, `show`, `clear` the saved default account |
| `lrfl login` / `logout` / `whoami` | Manage a logged-in session (refresh token in the OS keychain) |
| `lrfl profile` | Your account holder profile (name, email, phone) — requires login |
| `lrfl accounts` | Utility accounts linked to your login — requires login |
| `lrfl schedules` | Your scheduled payments — requires login |
| `lrfl wallet` | Your saved payment methods — requires login |
| `lrfl self-update` | Update `lrfl` to the latest GitHub release (`--check` to only check) |

### Logging in

Guest reads (`balance`, `charges`, `history`, …) need no login. The authenticated
commands (`profile`, `accounts`, `schedules`, `wallet`) use your portal account:

```sh
lrfl login                    # prompts for email + password (no-echo)
lrfl whoami                   # ✓ logged in as you@example.com
lrfl accounts                 # utility accounts on your login
lrfl profile --json
lrfl logout                   # removes the stored session
```

Login exchanges your email + password for an AWS Cognito token set. **Only the
long-lived refresh token is stored, in the OS keychain** (macOS Keychain) — never
your password, never the short-lived access token. Each authenticated command
trades the refresh token for a fresh access token at call time. `--email` /
`$LRFL_EMAIL` pick the account; `$LRFL_REFRESH_TOKEN` can supply the token in
headless/CI use. You can pipe the password on stdin (`echo "$PW" | lrfl login
--email you@example.com`) for scripting.

### Staying up to date

```sh
lrfl self-update --check      # is a newer release available?
lrfl self-update              # download + replace the binary in place
```

### Account numbers

A utility account is `NNNNNNN-N` — the base number and its check digit, exactly
as shown on your bill and in the portal URL (e.g. `1234567-0`).

### Paying

Card capture runs through the district's payment processor (BluePay/FIS) behind a
reCAPTCHA, so this tool does **not** handle card data. `lrfl pay` computes what
you owe and hands you the district's own secure Pay Now page for the account
(`--open` launches it). That keeps every payment on the official, PCI-compliant
flow.

## JSON & scripting

```sh
# Just the number you owe:
lrfl balance --json | jq -r '.balance_due'

# Total paid in the last 3 years:
lrfl history --json | jq '[.payments[].amount] | add'

# Machine-readable service status:
lrfl status --json | jq '{sewer, overall}'
```

In `--json` mode the JSON document is the only thing on **stdout**; diagnostics
go to **stderr**, so `| jq` is always safe.

## Privacy

The account owner's **name and mailing address are withheld by default** —
utility account numbers are sequential and easy to guess, so the tool won't print
someone's identity just because you typed a number. Pass `--show-owner` to reveal
them for an account you own. The district's own portal redacts owner names too;
this tool honors that.

The only secret the tool ever stores is your login **refresh token**, in the OS
keychain (never your password). Non-secret state — your default account number
and login email — lives in plain files under `~/.config/loxahatchee-cli/`. Guest
reads store nothing at all.

## Global flags

| Flag | Description |
|------|-------------|
| `--json` | Machine-readable JSON on stdout |
| `-v, --verbose` | Extra diagnostics on stderr |
| `-q, --quiet` | Suppress non-error stderr |
| `--show-owner` | Reveal owner name/address (hidden by default) |
| `--no-color` | Disable ANSI color (reserved) |
| `--wipp-id <ID>` | WIPP tenant id (default `LOXA`; or `$LRFL_WIPP_ID`) |
| `--email <EMAIL>` | Login email for authenticated commands (or `$LRFL_EMAIL`) |
| `-V, --version` | Print version |
| `-h, --help` | Help |

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | success |
| `1` | generic error (incl. keychain failure) |
| `2` | usage error (bad args / no account) |
| `3` | authentication required or failed |
| `4` | not found (no such account) |
| `5` | network / upstream error |
| `6` | rate-limited (HTTP 429) |
| `7` | timed out waiting on an async server request |

## How it works

The portal is a SunGard/Edmunds "WIPP" tenant (`LOXA`). Its API lives at
`api.edmundsgovtech.cloud/wipp-core/v1`, selects a tenant with the `X-Wipp-Id`
header, and (via an AWS WAF) expects a browser User-Agent. Account lookups are
plain guest reads; a couple of endpoints (like service status) are asynchronous —
you submit and poll `GET /requests/{id}` until it's ready. See
[`docs/wipp-api.md`](docs/wipp-api.md) for the discovered endpoints.

## Scope & disclaimer

A personal-use tool for viewing your own publicly available utility-account data
at human scale. Not affiliated with or endorsed by the Loxahatchee River
District, Edmunds GovTech, or SunGard. Endpoints are undocumented and may change.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your
option. See [`CONTRIBUTING.md`](CONTRIBUTING.md).
