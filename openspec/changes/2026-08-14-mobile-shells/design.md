# Mobile Shells — Design

## Repo layout

```
mobile/
├── capacitor.config.ts
├── package.json
├── tsconfig.json
├── src/
│   └── index.ts                 # Capacitor bootstrap
├── www/                         # populated by `cap copy`
├── ios/                         # `npx cap add ios`
└── android/                     # `npx cap add android`
```

The `mobile/` directory is a separate npm project. It does
NOT contain a separate web UI; `npx cap copy` pulls the
static assets from the Rust binary's `static/` directory into
`mobile/www/`, and `server.url` points the WebView at the
running binary.

## capacitor.config.ts

```ts
import type { CapacitorConfig } from '@capacitor/cli';

const config: CapacitorConfig = {
  appId: 'com.openaccounting.app',
  appName: 'OpenAccounting',
  webDir: 'www',
  server: {
    androidScheme: 'https',
    url: process.env.OPENACCOUNTING_SERVER_URL
         ?? 'https://localhost:3000',
  },
  plugins: {
    PushNotifications: {
      presentationOptions: ['badge', 'sound', 'alert'],
    },
  },
};
export default config;
```

## Build pipeline

`mobile/package.json`:

```json
{
  "scripts": {
    "sync": "cap sync",
    "build:android": "cap sync && cd android && ./gradlew assembleDebug",
    "build:ios":     "cap sync && cd ios && xcodebuild -workspace App.xcworkspace -scheme App -configuration Release"
  },
  "dependencies": {
    "@capacitor/core": "^6.0.0",
    "@capacitor/android": "^6.0.0",
    "@capacitor/ios": "^6.0.0",
    "@capacitor/push-notifications": "^6.0.0",
    "@capacitor-community/camera": "^6.0.0"
  }
}
```

## Push registration

```ts
// mobile/src/index.ts
import { PushNotifications } from '@capacitor/push-notifications';
import { Capacitor } from '@capacitor/core';

document.addEventListener('DOMContentLoaded', async () => {
  if (Capacitor.isNativePlatform()) {
    await PushNotifications.requestPermissions();
    await PushNotifications.register();
  }
});

PushNotifications.addListener('registration', async ({ value }) => {
  await fetch('/devices/register', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json',
               'X-CSRF-Token': csrf() },
    body: JSON.stringify({ token: value, platform: Capacitor.getPlatform() }),
  });
});

PushNotifications.addListener('pushNotificationReceived',
  (n) => {
    const url = n.data?.url;
    if (url) window.location.href = url;
  });
```

## Server-side notifications

```rust
// src/notifications/mod.rs
#[async_trait]
pub trait Notifier: Send + Sync {
    async fn push(&self, device: &DeviceToken,
        title: &str, body: &str, data: serde_json::Value)
        -> Result<(), NotifierError>;
}

pub struct ApnsNotifier { /* APNs cert path */ }
pub struct FcmNotifier  { /* FCM service account */ }
pub struct NoopNotifier {}
```

The dispatcher in `src/handlers/notifications.rs::dispatcher`
chooses the impl from `PUSH_PROVIDER`.

## Tests

### Build (CI)

- `npm install` succeeds.
- `npx cap sync` produces `mobile/www/index.html` from the
  Rust binary's static directory.
- `npm run build:android` produces a valid APK.

### Integration (server side)

- `http_devices_register_stores_token`.
- `http_dispatcher_with_noop_does_not_call_external` — `PUSH_PROVIDER=none`,
  no HTTP calls made.
- `http_dispatcher_pushes_to_registered_devices` — push is
  fanned out to every device the user has registered.

### Manual (mobile side)

- App installs on iOS Simulator (Xcode 15+).
- App installs on Android Emulator (API 33+).
- Push notification deep-links to the claim page.

## References

- Capacitor 6 docs (`capacitorjs.com`)
- APNs HTTP/2 API (`developer.apple.com/documentation/usernotifications`)
- FCM HTTP v1 API (`firebase.google.com/docs/cloud-messaging`)
