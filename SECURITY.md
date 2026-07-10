# Security policy

## Reporting a vulnerability

Please report security issues **privately** — do not open a public issue.

- Preferred: open a [GitHub private security advisory](https://docs.github.com/en/code-security/security-advisories)
  on this repo ("Security" tab → "Report a vulnerability").
- Or contact the maintainer.

We aim to acknowledge within a few days and coordinate a fix/disclosure with you.

## Notes for this tool

`lrfl` uses **no credentials** and stores **no secrets** — it makes anonymous,
read-only guest-view requests to a public utility portal, and (optionally) saves
a default account number, which is not a credential, in a plain config file.

The main concerns are therefore:

- **Not introducing secret- or PII-handling by accident.** Owner name/address is
  redacted by default; tests and fixtures use synthetic data; the pre-commit hook
  runs `gitleaks`.
- **Dependency advisories** (`cargo audit` / `cargo deny`).
- **Payments stay on the official gateway.** This tool never captures card data;
  it hands off to the district's PCI-compliant Pay Now page.

## Supported versions

Pre-1.0: only the latest release receives fixes.
