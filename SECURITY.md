# Security policy

## Reporting a vulnerability

Please report security issues **privately** — do not open a public issue.

- Preferred: open a [GitHub private security advisory](https://docs.github.com/en/code-security/security-advisories)
  on this repo ("Security" tab → "Report a vulnerability").
- Or contact the maintainer.

We aim to acknowledge within a few days and coordinate a fix/disclosure with you.

## Notes for this tool

Guest reads are anonymous and store nothing. If you `lrfl login`, the tool holds
one secret: your portal **refresh token**, stored in the **OS keychain** (macOS
Keychain / Windows Credential Manager / Linux Secret Service). It never stores
your password or the short-lived access token, and never writes a credential to
disk. Non-secret state (default account number, login email) lives in plain
`~/.config` files.

The main concerns are therefore:

- **Credential handling.** Secrets are wrapped in a type that refuses to print
  itself (`Debug`/`Display` redacted) and is zeroized on drop; nothing secret is
  logged or written to disk. `$LRFL_REFRESH_TOKEN` is an env fallback for CI.
- **No secrets or PII in the repo.** Owner name/address is redacted by default;
  tests and fixtures use synthetic data; the pre-commit hook runs `gitleaks`.
- **Dependency advisories** (`cargo audit` / `cargo deny`).
- **Payments stay on the official gateway.** This tool never captures card data;
  it hands off to the district's PCI-compliant Pay Now page.

## Supported versions

Pre-1.0: only the latest release receives fixes.
