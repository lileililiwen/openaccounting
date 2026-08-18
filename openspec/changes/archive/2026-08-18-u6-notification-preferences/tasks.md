## 1. Testing

- [x] 1.1 HTTP: `http_notifications_defaults`.
- [x] 1.2 HTTP: `http_notifications_toggle_persists`.
- [x] 1.3 Unit: `notification_respects_preference`.

## 2. Implementation

- [x] 2.1 `migrations/0032_add_notification_preferences.sql`.
- [x] 2.2 `src/handlers/notification_preferences.rs`.
- [x] 2.3 `src/notifications/preferences.rs`.

## 3. Validation

- [x] 3.1 `openspec validate u6-notification-preferences`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --all-targets --features test-support`.
- [x] 3.4 `cargo test --features test-support`.
- [x] 3.5 `openspec archive u6-notification-preferences`.