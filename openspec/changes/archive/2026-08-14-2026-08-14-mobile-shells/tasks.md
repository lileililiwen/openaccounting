# Mobile Shells — Tasks

## 1. Testing

- [x] 1.1 Build: `npm install` inside `mobile/` succeeds.
- [x] 1.2 Build: `npx cap sync` produces `mobile/www/index.html`.
- [x] 1.3 Build: `npm run build:android` produces a valid APK.
- [ ] 1.4 Manual: install on iOS Simulator (Xcode 15+) —
      app opens the configured `server.url`.
- [ ] 1.5 Manual: install on Android Emulator (API 33+) —
      app opens the configured `server.url`.
- [x] 1.6 Integration (server): `http_devices_register_stores_token`.
- [x] 1.7 Integration (server):
      `http_dispatcher_with_noop_does_not_call_external`.
- [x] 1.8 Integration (server):
      `http_dispatcher_pushes_to_registered_devices`.

## 2. Implementation

- [x] 2.1 Add `apns-rust = "0.2"`, `fcm = "0.10"` (feature
      flags) to `Cargo.toml` (stubs in v1; no native deps).
- [x] 2.2 Migration `0025_add_device_tokens.sql`.
- [x] 2.3 `src/handlers/notifications.rs` — register,
      unregister, dispatcher.
- [x] 2.4 `src/notifications/{apns,fcm,noop}.rs` — impls of
      the `Notifier` trait.
- [x] 2.5 `src/lib.rs` — register notification routes.
- [x] 2.6 `src/config.rs` — `push_provider`,
      `apns_key_path`, `fcm_service_account_path` (env-based).
- [x] 2.7 `.env.example` — document new vars.
- [x] 2.8 `mobile/` directory: capacitor.config.ts,
      package.json, tsconfig.json, src/index.ts.
- [x] 2.9 `mobile/src/index.ts` — PushNotifications + Camera
      plugins.
- [ ] 2.10 Generate iOS app icons in all required sizes.
- [x] 2.11 `.github/workflows/mobile-build.yml` — builds on tag.

## 3. Validation

- [x] 3.1 `openspec validate mobile-shells` passes.
- [x] 3.2 `cargo fmt --check` clean.
- [x] 3.3 `cargo clippy --all-targets -- -D warnings` clean.
- [x] 3.4 `cargo test` green (61 integration tests passing).
- [ ] 3.5 Manual smoke: launch app, login, navigate around,
      receive a test push, tap the notification, confirm
      deep-link.
- [x] 3.6 `openspec archive mobile-shells`.
