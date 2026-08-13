use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct AuditEntry {
    pub id: Uuid,
    pub ledger_id: Option<Uuid>,
    pub actor_id: Uuid,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub created_at: DateTime<Utc>,
}

pub async fn log(
    pool: &PgPool,
    ledger_id: Option<Uuid>,
    actor_id: Uuid,
    action: &str,
    entity_type: &str,
    entity_id: Option<Uuid>,
    old_value: Option<Value>,
    new_value: Option<Value>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO audit_entries (ledger_id, actor_id, action, entity_type, entity_id, old_value, new_value)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(ledger_id)
    .bind(actor_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(old_value)
    .bind(new_value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list(
    pool: &PgPool,
    ledger_id: Option<Uuid>,
    actor_id: Option<Uuid>,
    action: Option<&str>,
    entity_type: Option<&str>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    limit: i64,
    offset: i64,
) -> Result<Vec<AuditEntry>, sqlx::Error> {
    let entries = sqlx::query_as::<_, AuditEntry>(
        r#"
        SELECT id, ledger_id, actor_id, action, entity_type, entity_id, old_value, new_value, created_at
        FROM audit_entries
        WHERE ($1::uuid IS NULL OR ledger_id = $1)
          AND ($2::uuid IS NULL OR actor_id = $2)
          AND ($3::text IS NULL OR action = $3)
          AND ($4::text IS NULL OR entity_type = $4)
          AND ($5::timestamptz IS NULL OR created_at >= $5)
          AND ($6::timestamptz IS NULL OR created_at <= $6)
        ORDER BY created_at DESC
        LIMIT $7 OFFSET $8
        "#,
    )
    .bind(ledger_id)
    .bind(actor_id)
    .bind(action)
    .bind(entity_type)
    .bind(from)
    .bind(to)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(entries)
}
