DROP TABLE IF EXISTS notification_http_targets;
DROP TABLE IF EXISTS in_app_notifications;
DROP INDEX IF EXISTS uq_txn_template_due;
DROP TABLE IF EXISTS invoice_reminders;
DROP TABLE IF EXISTS webhook_deliveries;
DROP TABLE IF EXISTS webhook_subscriptions;
DROP TABLE IF EXISTS jobs;
ALTER TABLE notification_preferences DROP CONSTRAINT IF EXISTS notification_preferences_event_check;
ALTER TABLE notification_preferences ADD CONSTRAINT notification_preferences_event_check
    CHECK (event IN (
        'budget_overrun',
        'reimbursement_submitted',
        'large_transaction',
        'weekly_summary'
    ));
