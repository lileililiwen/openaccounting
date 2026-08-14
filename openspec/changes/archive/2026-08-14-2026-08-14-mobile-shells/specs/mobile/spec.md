# mobile Specification

## Purpose

Define the native mobile shells (iOS, Android) that wrap the
existing web UI via Capacitor.

## ADDED Requirements

### Requirement: Capacitor Project

The repo SHALL contain a `mobile/` directory at the root with
a complete Capacitor project:

- `mobile/capacitor.config.ts` declaring `appId="com.openaccounting.app"`,
  `appName="OpenAccounting"`, `webDir="www"`, and a
  `server.url` configurable per build (`server.androidScheme="https"`).
- `mobile/package.json` with the Capacitor CLI and the
  `PushNotifications` plugin as dependencies.
- `mobile/www/` initially empty — populated by
  `npx cap copy` (which copies from the running server).

#### Scenario: Project builds locally

- **WHEN** a developer runs `npm install` inside `mobile/`
  followed by `npm run build:android`
- **THEN** an APK is produced at
  `mobile/android/app/build/outputs/apk/debug/app-debug.apk`.

### Requirement: Server URL Configuration

The mobile app reads its target server URL from a single
build-time env var `OPENACCOUNTING_SERVER_URL` (default
`https://localhost:3000` for dev). The Capacitor config
embeds this URL into `server.url` at build time.

#### Scenario: Switching servers

- **WHEN** a developer builds with
  `OPENACCOUNTING_SERVER_URL=https://demo.openaccounting.org`
- **THEN** the app opens that URL on launch instead of
  localhost.

### Requirement: Native App Icons and Splash

The mobile project SHALL include:

- App icons in all required iOS sizes (20, 29, 40, 58, 60, 76,
  80, 87, 120, 152, 167, 180 px) generated from
  `static/icons/icon-512.png`.
- A splash screen with the app logo and slate-900 background.

#### Scenario: Icons render in iOS

- **WHEN** the app is installed on iOS
- **THEN** the home-screen icon displays the OpenAccounting
  logo on the slate-900 background.

### Requirement: Push Notification Hooks

The mobile app SHALL:

1. Register for push notifications on login
   (`PushNotifications.register()`).
2. POST the resulting FCM/APNs token to
   `/devices/register` on the server.
3. Handle incoming push notifications by navigating the
   in-app WebView to the relevant URL (deep link).

#### Scenario: Push notification deep-links to a claim

- **WHEN** the server pushes "Reimbursement #123 awaiting
  your approval" to the device
- **THEN** tapping the notification opens the app and
  navigates to `/ledgers/X/reimbursements/123`.

### Requirement: Camera Capture for Receipts

The mobile app SHALL expose a "Camera" button on the new
reimbursement-line form that uses
`@capacitor-community/camera` to take a photo and attach it
via the existing document upload endpoint.

#### Scenario: Camera receipt upload

- **WHEN** the user taps "Camera" on the line form and
  captures a receipt
- **THEN** the photo is uploaded via `POST
  /ledgers/{id}/documents` with `claim_line_id=<id>` and
  the document appears attached to the line.

### Requirement: Same Web UI

The mobile app SHALL use the same server-rendered HTML as the
desktop web app. There SHALL NOT be a separate mobile UI.

#### Scenario: Layout adapts

- **WHEN** the user opens the app on an iPhone SE (small
  screen)
- **THEN** the responsive Tailwind layout from
  `2026-08-14-bootstrap-double-entry-bookkeeping-engine`
  renders the same content in mobile mode.

---

# notifications Specification

## Purpose

Define the server-side push notification pipeline.

## ADDED Requirements

### Requirement: Device Registration

`POST /devices/register` MUST accept `{ token: string, platform:
"ios" | "android" }` and store it in `device_tokens`
(`id, user_id, token, platform, last_seen_at`). The handler
SHALL require authentication.

#### Scenario: User registers their device

- **WHEN** an authenticated user POSTs
  `{"token":"abc","platform":"ios"}`
- **THEN** a row is added with `user_id=<user.id>` and the
  response is `201 Created`.

### Requirement: Push on Approval Required

When a reimbursement claim's required-approval set changes
(due to a new policy or new approver), the server SHALL push
a notification to every approver's registered devices with
title `"Reimbursement awaiting your approval"`, body
`"<title> (#<short id>)"`, and data `{ url:
"/ledgers/{id}/reimbursements/{claim_id}" }`.

#### Scenario: Approver receives notification

- **WHEN** a claim transitions to `submitted` and Alice has
  `role=Admin` and one registered device
- **THEN** Alice's device receives a push within 30 seconds
  with the claim title and short id in the body.

### Requirement: Push on Sync Done

When a bank-feed sync completes and adds N new transactions,
the server SHALL push a notification to the ledger owner with
title `"Bank feed sync"`, body `"<N> new transactions on
<link_nickname>"`.

#### Scenario: User receives sync notification

- **WHEN** a Plaid link syncs 5 new transactions
- **THEN** the ledger owner receives a push within 60 seconds.

### Requirement: Push Provider Pluggability

The notifications module SHALL expose a `Notifier` trait with two
implementations in v1: `ApnsNotifier` (iOS) and
`FcmNotifier` (Android). The implementation MUST be selected by
`PUSH_PROVIDER` env var (`apns | fcm | none`); `none` SHALL disable
pushes without breaking the routes.

#### Scenario: Push disabled in dev

- **WHEN** `PUSH_PROVIDER=none` is set
- **THEN** the `/devices/register` route still works (stores
  the token) but the dispatcher no-ops; no external calls are
  made.
