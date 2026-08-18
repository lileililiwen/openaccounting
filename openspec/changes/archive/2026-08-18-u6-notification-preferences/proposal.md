# Notification Preferences

## Why

The notifications capability ships push (`migrations/0025`) but no UI
to opt in or out. Users get whatever the system sends. Firefly III has a
notification preferences page.

## What Changes

- New `notification_preferences` table `(user_id, channel, event,
  enabled)`.
- Channels: email, push, in-app.
- Events: budget_overrun, reimbursement_submitted, large_transaction,
  weekly_summary.
- Page at `/account/notifications`.
- Honor the preference everywhere a notification is sent.

## Capabilities

### New Capabilities

- `notification-preferences`: Per-user channel/event opt-ins.

## Impact

**New files:**
- `migrations/0043_add_notification_preferences.sql`.
- `src/handlers/notification_preferences.rs`.
- `src/notifications/preferences.rs`.
