## 1. Testing

- [x] 1.1 HTTP: `http_theme_default_is_system`.
- [x] 1.2 HTTP: `http_theme_toggle_persists`.
- [ ] 1.3 Manual screenshot — deferred (no browser test harness; the structural HTTP tests cover the server side).

## 2. Implementation

- [x] 2.1 `migrations/0031_add_user_theme.sql`.
- [x] 2.2 `static/css/app.css` — dark variants (scrollbar, HTMX indicator).
- [x] 2.3 `templates/base.html` — toggle script + `dark:` body classes.
- [x] 2.4 `src/handlers/account_theme.rs`.

## 3. Validation

- [x] 3.1 `openspec validate u8-dark-mode`.
- [x] 3.2 `cargo fmt --check`.
- [x] 3.3 `cargo clippy --all-targets --features test-support`.
- [x] 3.4 `cargo test --features test-support`.
- [x] 3.5 `openspec archive u8-dark-mode`.