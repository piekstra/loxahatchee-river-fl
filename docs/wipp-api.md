# WIPP (Edmunds/SunGard) utility API — discovered notes

Reverse-engineered from the public payment portal's own JavaScript bundle and
its anonymous guest-view calls (the owner's own account). **No secrets or PII
here** — account numbers, names, addresses, and balances are deliberately
omitted. This is the spec `lrfl` implements.

> Discovered 2026-07-10. Undocumented private API; treat as unstable — shapes and
> the tenant configuration change with deploys.

## Basics

- **Base:** `https://api.edmundsgovtech.cloud/wipp-core/v1`
- **Tenant selector:** header `X-Wipp-Id: LOXA` — required on essentially every
  call (Loxahatchee River District is tenant `LOXA`).
- **WAF gate:** an AWS WAF fronts the API and **rejects non-browser
  `User-Agent`s** — a bare client string returns `403 {"message":"Forbidden"}`.
  A browser-shaped UA (a polite tool suffix is fine) passes. `Origin` is *not*
  required once the UA is browser-like.
- Responses are JSON on success; some errors come back as a **bare string**
  `(NNN) message` rather than a JSON object, so parse defensively.

## Account id format

The portal shows a utility account as `NNNNNNN-N` (base + check digit). The API
wants a fixed-width, space-padded 11-char key, then percent-encoded into the
path:

```
display  1234567-0
raw      " 1234567  0"   = base.rjust(8, ' ') + check.rjust(3, ' ')
encoded  %201234567%20%200
```

## Async request pattern (important)

Some mainframe-backed operations don't answer inline. They return
`202 { "requestId": "<uuid>" }`; you then **poll `GET /requests/{requestId}`**,
which stays `202` while processing and flips to `200` with the result body when
done. A health check lives at `GET /requests/ping` (send `X-Wipp-Id`). `lrfl`
polls with a short interval and a ~20s budget.

## Guest (no-auth) endpoints — implemented

These power `lrfl`'s read commands. All take `X-Wipp-Id` + a browser UA.

### Tenant configuration
- `GET /metadata/{wippId}` → district name, state, which services are
  installed/metered/payable (`wtr`/`swr`/`elc`/`otr` flags), overpayment limit,
  contact message, merchant codes. Powers `lrfl district`.
- `GET /capabilities/{wippId}` → `{ utilBill, njTaxBill, arStandardInvoice, … }`
  booleans for which bill types the tenant accepts.
- `GET /wipp/additional-metadata/{wippId}` → `{ featureFlags{…},
  disallowedPaymentMethods[] }`. LOXA flags include `RedactOwnerName` (the API
  blanks owner names), `OverrideQuickpayUrl`, `AllowBankruptPayment`,
  `AllowPaymentLessThanInterest`, `ThirdPartyPDFPrint`.

### Account lookup  → powers `account` / `balance` / `charges`
`GET /wippUtil/{encodedAccountId}` → the full utility account:
```
id, billToName, serviceLoc, propertyLoc,
utilityOwnerInfo{ name, street1, street2, cityState, zip },
propertyInfo{ … assessment/owner fields … },
chargeTypes: { "<ServiceName padded>": {
   totPrnBal, totIntDue, futurePrnBal, otrDelqPrnBal, otrDelqIntDue,
   billedYtd, currDueDate, lastDatePaid,
   currRdg, currRdgDate, currUsage, currPrdStartDate, currPrdEndDate,
   currDiscAmt, currDiscDate,
   prior{DueDate,Rdg,RdgDate,Usage,PrdBilled,PrdPrnBal,PrdInt}{1,2,3}, … } }
```
Not found / no PIN → `401`/`404`.

**Amount due** (per service, matching the portal's account-balance math):
```
amount_due(service) = totPrnBal + totIntDue − futurePrnBal
balance_due         = Σ amount_due(service)
```
(The portal also has a per-service early-pay discount term, `currDiscAmt`, which
is surfaced in `charges` but not added into the account balance — the SPA's
account-level `calcUtilityBalance` predicate doesn't fire on the raw field.)

### Service status  → powers `status`  (async)
`GET /wippUtil/{id}/determineAccountStatus` → `202 {requestId}` → poll →
`{ accountStatus, wtrStatus, swrStatus, elcStatus, otrStatus }` with one-letter
codes (`A` active, `N` none).

### Payment history  → powers `history`
`GET /billingAccounts/wippUtil/{id}/payments?paymentsAfterDate=YYYY-MM-DD`
→ `[{ wippId, transactionId, accountType, accountId, amt, paymentMethodCode,
postTime, paymentDate, userPart3/4/5 }]`. The entity segment is the literal
`wippUtil` (not `U`/`UTILITY` — those 400 with "No enum constant").

### Paying
Card capture is **not** a plain JSON POST: it runs through the tenant's processor
(BluePay via `…/proxy/bluepay/…`, or FIS/Link2Gov quick-pay) and is **gated by
reCAPTCHA** (only `/proxy/bluepay/` and `/payments/schedules/one-time` require
the `x-recaptcha-token` header). So `lrfl pay` deliberately hands off to the
portal's Pay Now page — `https://wipp.edmundsgovtech.cloud/view/wippUtil/{dashed
id}?wippId=LOXA` — rather than handling a card.

## Authenticated endpoints — discovered, not yet implemented

Logging in unlocks a wallet, autopay, and scheduled payments. These need a
Cognito session and, in places, reCAPTCHA, so they're a roadmap rather than
shipped commands. Recorded here so the surface is known:

- **Auth (AWS Cognito):** `POST /auth` `{email, password}` → session with
  `cognitoToken{accessToken, refreshToken}`; `POST /auth/refreshToken`,
  `/forgotPassword`, `/resetForgottenPassword`, `/changePassword`. Authenticated
  calls add `Authorization: Bearer <accessToken>` + `X-Wipp-Id`.
- **Profile:** `GET/POST /accounts/cognitoUsers` (`/add`, update, delete),
  returns `{firstName, lastName, email, phoneNumber}`.
- **Linked billing accounts:** `…/accounts/billingAccounts/{type}/{id}` and
  `/accounts/billingAccounts/{type}/{id}/autopay/{billingGroupId}`.
- **Wallet / payment methods:** `GET/POST/DELETE /wallet/Accounts`,
  `/wallet/PaymentMethods`, `/wallet/{id}`.
- **Autopay:** `GET {base}/autopay/associatedAccounts?methodId={id}`.
- **Scheduled payments:** `GET /payments/schedules`, `POST /payments/schedules/
  one-time`, `PUT /payments/schedules/one-time/{id}` (reCAPTCHA-gated).

If these get built, auth should follow the project's usual model: the Cognito
**refresh token in the OS keychain**, email as a non-secret default, never a
plaintext credential on disk.
