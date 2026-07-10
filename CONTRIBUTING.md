# Contributing to loxahatchee-river-fl

Thanks for helping out! This is a small, focused CLI over the Loxahatchee River
District's WIPP utility-billing portal.

## Before you start

- Open or comment on an issue describing the change.
- Branch from `main` (`feat/…`, `fix/…`, `docs/…`); don't push to `main` directly.
- Use [Conventional Commits](https://www.conventionalcommits.org/).
- Enable the pre-commit hook once: `git config core.hooksPath .githooks`.

## Local checks (must pass)

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

If you touch API parsing, add/adjust a unit test with a **synthetic** JSON node
(see `src/account.rs`). Keep fields best-effort — the mainframe responses drift
and a missing field shouldn't crash a lookup.

## Ground rules

- **No secrets or PII** in code, tests, fixtures, or commits — real account
  numbers, names, addresses, or balances included. This tool needs no
  credentials; keep it that way. `gitleaks` runs in the pre-commit hook.
- Owner name/address stays **redacted by default** (`--show-owner` to reveal).
- Card payments go through the district's official gateway — don't add code that
  captures or posts card data.
- Keep it personal-scale and polite to the portal. No bulk scraping or hammering.
- Be kind — see [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

## License of contributions

By contributing, you agree your work is dual-licensed under **MIT OR Apache-2.0**.
