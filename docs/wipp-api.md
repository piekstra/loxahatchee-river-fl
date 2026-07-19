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
propertyInfo{ interestDate, …assessment/owner fields… },  // interestDate powers "Interest thru"
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

### Address search  → powers `search`
`GET /wippUtil/search?propertyLoc=<query>&size=<n>` → a Spring page of accounts
matching a **street/property location**. The district matches **server-side**
(case-insensitive **substring** on the property location) — no geocoding or third
party. Anonymous (no login).
```
{ content: [ { wippId, accountId (space-padded), chargeType,
               propertyLoc, ownerName, billToName, propLocStDirNum, … } ],
  totalElements, totalPages, size, number, … }
```
- The recognized filter key is **`propertyLoc`**; unknown keys (`streetName`,
  `address`, `accountNumber`, …) are silently ignored and the endpoint returns an
  unfiltered first page — so verify a query actually filters.
- **Gotcha:** `totalElements` is **page-capped** — it equals the number of rows
  returned, not the true match count. So there's no reliable total; a *full* page
  just means "there may be more" (raise `size`). `lrfl search` surfaces this as a
  `truncated` flag rather than a bogus "of N".
- The search URL segment is `wippUtil` for utility; tax/property search uses a
  different segment (`WippPropInfo`) that LOXA (sewer-only) doesn't expose.
- The result rows carry `ownerName`/`billToName` and a nested `wippPropInfo`
  (assessment/owner), but **no balance or charges** — and for LOXA those owner
  and assessment fields come back blank. `lrfl search --balances` therefore fans
  out one `/wippUtil/{id}` lookup per match to attach each account's balance.

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

## Authenticated endpoints (verified against a live login)

Loxahatchee accounts authenticate through the **SunGard / FIS ("Link2Gov")**
identity provider (`fisVersion: 2`), proxied by wipp-core — **not** the Cognito
`/auth` path that some other WIPP tenants use. Login is two hops over one cookie
session:

1. `POST {FIS_PROXY}/rest/1.0/sessions` — body `{ loginName, password }`, header
   `api-type: auth`. On success → `200` and **Set-Cookie** session cookies (empty
   body). Wrong credentials → `400` (occasionally `401`/`500`), no useful body.
2. `GET {FIS_PROXY}/rest/1.0/idptoken/openid-connect?client_id=Enroll.User` (with
   the session cookies) → `{ "id_token": "<JWT>" }`.

`FIS_PROXY` = `https://api.edmundsgovtech.cloud/wipp-core/proxy/fis`. The
`id_token` is a short-lived JWT; its claims are `{ sub (login), UID, FirstName,
UserType, LastLoginDate, exp, iat, iss, aud, jti, nbf }`. Use it as
`Authorization: Bearer <id_token>` (+ `X-Wipp-Id`) for authenticated wipp-core
calls. `lrfl` stores the **password** (keychain) and re-runs both hops per command
to mint a fresh token — the FIS session is cookie-based with no client-visible
refresh token to persist. → powers `login` / `logout` / `whoami`.

- **Linked accounts:** `GET /accounts/billingAccounts` → `[{ wippId, fisUserId,
  accountType, accountId }]` (the `accountId` is the space-padded key). **Works
  for FIS users.** → powers `accounts`.

**Cognito-only endpoints (403/401/500 for FIS users, so not shipped):**
`GET /accounts/cognitoUsers` (profile) → `403 Access denied`;
`GET /payments/schedules` and `GET /wallet/Accounts` → `500` (or `401` with an
`api-type`). These belong to the Cognito user model / features LOXA doesn't expose
to FIS logins. Also present but not wired (write paths, several reCAPTCHA-gated):
`POST /payments/schedules/one-time`, wallet management, autopay enrollment.
