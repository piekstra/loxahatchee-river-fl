# Changelog

## v0.3.0 — 2026-07-19

### Added
- **`lrfl search <address>`** — find accounts by street/property address. The
  district matches server-side (case-insensitive substring on the property
  location); no login and no external geocoding are involved. `--limit N` caps
  results; `--json` emits `{ query, count, truncated, matches }`.
- **Full per-service detail** in the human `account` view: principal, interest,
  not-yet-due principal, billed-YTD, last paid, billing period, meter
  reading/usage, and early-pay discount — plus the **interest-through date**
  (`propertyInfo.interestDate`). This data was already in `--json`; now it's
  surfaced in human output too.

### Changed
- **Owner name and mailing address are shown by default.** The CLI faithfully
  renders whatever the portal returns and no longer imposes privacy the provider
  itself doesn't (the portal already serves anonymous account-number lookups and
  blanks fields on its own via `RedactOwnerName`).
- Docs corrected to state the honest reason `pay` hands off — the portal exposes
  no programmatic card API (reCAPTCHA + a processor-hosted card form) — rather
  than any PCI-compliance or repo-privacy framing.

### Removed
- **BREAKING:** the `--show-owner` global flag. Owner details are shown by
  default now, so the flag (and the redaction path behind it) is gone.

## v0.2.1 — 2026-07-11
- Homebrew formula and release polish.

## v0.2.0 — 2026-07-11
- De-duplicated the auth commands; breaking CLI cleanups toward piekstra-cli/1.

## v0.1.0 — 2026-07-11
- Initial release: anonymous guest-view account/billing/charges/history, the
  SunGard/FIS login flow, and `self-update` from GitHub releases.
