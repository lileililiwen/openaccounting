# Mobile Shells — Tasks

## 1. Testing

- [ ] 1.1 Build: `npm install` inside `mobile/` succeeds.
- [ ] 1.2 Build: `npx cap sync` produces `mobile/www/index.html`.
- [ ] 1.3 Build: `npm run build:android` produces a valid
      APK.
- [ ] 1.4 Manual: install on iOS Simulator (Xcode 15+) —
      app opens the configured `server.url`.
- [ ] 1.5 Manual: install on Android Emulator (API 33+) —
      app opens the configured `server.url`.
- [ ] 1.6 Integration (server): `http_devices_register_stores_token`.
- [ ] 1.7 Integration (server):
      `http_dispatcher_with_noop_does_not_call_external`.
- [ ] 1.8 Integration (server):
      `http_dispatcher_pushes_to_registered_devices`.

## 2. Implementation

- [ ] 2.1 Add `apns-rust = "0.2"`, `fcm = "0.10"` (feature
      flags) to `Cargo.toml`.
- [ ] 2.2 Migration `0025_add_device_tokens.sql`.
- [ ] 2.3 `src/handlers/notifications.rs` — register,
      unregister, dispatcher.
- [ ] 2.4 `src/notifications/{apns,fcm,noop}.rs` — impls of
      the `Notifier` trait.
- [ ] 2.5 `src/main.rs` — register notification routes,
      start dispatcher.
- [ ] 2.6 `src/config.rs` — `push_provider`,
      `apns_key_path`, `fcm_service_account_path`.
- [ ] 2.7 `.env.example` — document new vars.
- [ ] 2.8 `mobile/` directory: capacitor.config.ts,
      package.json, tsconfig.json, src/index.ts, ios/,
      android/.
- [ ] 2.9 `mobile/src/index.ts` — PushNotifications + Camera
      plugins.
- [ ] 2.10 Generate iOS app icons in all required sizes.
- [ ] 2.11 `.github/workflows/mobile-build.yml` — builds on
      tag.

## 3. Validation

- [ ] 3.1 `openspec validate mobile-shells` passes.
- [ ] 3.2 `cargo fmt --check` clean.
- [ ] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [ ] 3.4 `cargo test` green.
- [ ] 3.5 Manual smoke: launch app, login, navigate around,
      receive a test push, tap the notification, confirm
      deep-link.
- [ ] 3.6 `openspec archive mobile-shells`.
