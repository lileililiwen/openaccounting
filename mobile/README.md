# OpenAccounting Mobile

Native iOS and Android shells for the OpenAccounting web app, built
with [Capacitor](https://capacitorjs.com/).

## Prerequisites

- Node.js 18+
- For iOS: Xcode 15+ (macOS only)
- For Android: Android Studio / SDK

## Setup

```bash
npm install
npx cap sync
```

## Build

```bash
# Android debug APK
npm run build:android

# iOS (macOS only, opens Xcode)
npm run build:ios
```

## Configuration

Set `OPENACCOUNTING_SERVER_URL` before building to point at your server:

```bash
OPENACCOUNTING_SERVER_URL=https://app.example.com npm run build:android
```

## Push Notifications

The app registers for push notifications on first launch. The token
is posted to `/devices/register` on your OpenAccounting server.

- **iOS**: Requires an APNs key (`APNS_KEY_PATH`, `APNS_KEY_ID`,
  `APNS_TEAM_ID`, `APNS_BUNDLE_ID`).
- **Android**: Requires a Firebase service account
  (`FCM_SERVICE_ACCOUNT_PATH`).
- **Self-hosted without push**: set `PUSH_PROVIDER=none` (default).
