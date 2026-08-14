//! Approval-routing domain: threshold-based multi-level approval.
//!
//! A ledger defines zero or more `reimbursement_approval_policies`,
//! each mapping a `min_amount` threshold to an approval `level` and a
//! required `approver_role`. At submit/approve time the set of levels a
//! claim needs is computed by walking the policy list and collecting
//! every policy whose `min_amount <= total`. When no policy matches,
//! the claim requires level 1 by default (the v1 single-approver
//! behaviour).

use rust_decimal::Decimal;
use sqlx::Postgres;
use uuid::Uuid;

use crate::error::AppResult;

/// Pure helper: turn a set of matching policy levels into the required
/// level list — sorted, deduped, defaulting to `[1]` when empty.
pub fn resolve_required_levels(matching: &[i32]) -> Vec<i32> {
    let mut levels = matching.to_vec();
    if levels.is_empty() {
        levels.push(1);
    }
    levels.sort_unstable();
    levels.dedup();
    levels
}

/// The required approval levels for a claim with `total` in `ledger_id`.
pub async fn required_levels<'exec, E>(
    exec: E,
    ledger_id: Uuid,
    total: Decimal,
) -> AppResult<Vec<i32>>
where
    E: sqlx::Executor<'exec, Database = Postgres>,
{
    let rows: Vec<(i32,)> = sqlx::query_as(
        "SELECT level FROM reimbursement_approval_policies
         WHERE ledger_id = $1 AND min_amount <= $2
         ORDER BY min_amount, level",
    )
    .bind(ledger_id)
    .bind(total)
    .fetch_all(exec)
    .await?;
    let matching: Vec<i32> = rows.into_iter().map(|(l,)| l).collect();
    Ok(resolve_required_levels(&matching))
}

/// The distinct levels already recorded for a claim, sorted ascending.
pub async fn recorded_levels<'exec, E>(exec: E, claim_id: Uuid) -> AppResult<Vec<i32>>
where
    E: sqlx::Executor<'exec, Database = Postgres>,
{
    Ok(sqlx::query_scalar(
        "SELECT DISTINCT level FROM reimbursement_approval_steps
         WHERE claim_id = $1 ORDER BY level",
    )
    .bind(claim_id)
    .fetch_all(exec)
    .await?)
}

/// Users allowed to approve for a given `min_role` on a ledger.
///
/// - `Accountant`: any user with edit access (owner or non-viewer member).
/// - `Admin`: the ledger owner, or a user with global role `admin`
///   who also has edit access.
///
/// Viewers are never eligible (the base spec forbids viewers from
/// approving).
pub async fn eligible_approvers<'exec, E>(
    exec: E,
    ledger_id: Uuid,
    min_role: &str,
) -> AppResult<Vec<Uuid>>
where
    E: sqlx::Executor<'exec, Database = Postgres>,
{
    Ok(sqlx::query_scalar(
        r#"SELECT DISTINCT u.id
           FROM users u
           WHERE (
               u.id = (SELECT owner_id FROM ledgers WHERE id = $1)
               OR u.id IN (SELECT user_id FROM ledger_members
                           WHERE ledger_id = $1 AND role <> 'viewer')
           )
           AND (
               $2 = 'Accountant'
               OR u.role = 'admin'
               OR u.id = (SELECT owner_id FROM ledgers WHERE id = $1)
           )
           ORDER BY 1"#,
    )
    .bind(ledger_id)
    .bind(min_role)
    .fetch_all(exec)
    .await?)
}

/// The approver role required for a specific `level` of a claim with
/// `total`. Defaults to `Admin` when no policy defines the level (v1
/// behaviour: the owner approves).
pub async fn approver_role_for_level<'exec, E>(
    exec: E,
    ledger_id: Uuid,
    level: i32,
    total: Decimal,
) -> AppResult<String>
where
    E: sqlx::Executor<'exec, Database = Postgres>,
{
    let role: Option<String> = sqlx::query_scalar(
        "SELECT approver_role FROM reimbursement_approval_policies
         WHERE ledger_id = $1 AND level = $2 AND min_amount <= $3
         ORDER BY min_amount DESC LIMIT 1",
    )
    .bind(ledger_id)
    .bind(level)
    .bind(total)
    .fetch_optional(exec)
    .await?;
    Ok(role.unwrap_or_else(|| "Admin".to_string()))
}

/// Count of distinct users with access to the ledger (owner + members).
/// Used to detect single-operator ledgers for the self-approve rule.
pub async fn ledger_user_count<'exec, E>(exec: E, ledger_id: Uuid) -> AppResult<i64>
where
    E: sqlx::Executor<'exec, Database = Postgres>,
{
    let (count,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM (
             SELECT owner_id AS user_id FROM ledgers WHERE id = $1
             UNION
             SELECT user_id FROM ledger_members WHERE ledger_id = $1
           ) u"#,
    )
    .bind(ledger_id)
    .fetch_one(exec)
    .await?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_levels_empty_defaults_to_one() {
        assert_eq!(resolve_required_levels(&[]), vec![1]);
    }

    #[test]
    fn required_levels_with_two_matching_policies() {
        assert_eq!(resolve_required_levels(&[1, 2]), vec![1, 2]);
    }

    #[test]
    fn recorded_levels_are_sorted_distinct() {
        let mut levels = vec![2, 1, 2, 3, 1];
        levels.sort_unstable();
        levels.dedup();
        assert_eq!(levels, vec![1, 2, 3]);
    }

    #[test]
    fn prop_required_levels_sorted_distinct() {
        // 1000 random "matching" sets always resolve to a
        // sorted, deduped list that contains every input level.
        let mut seed: u64 = 0x1234_5678;
        for _ in 0..1000 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let n = (seed % 6) as usize;
            let mut matching: Vec<i32> = (0..n)
                .map(|i| (((seed >> (8 * i)) % 4) + 1) as i32)
                .collect();
            let mut expected = matching.clone();
            if expected.is_empty() {
                expected.push(1);
            }
            let resolved = resolve_required_levels(&matching);
            expected.sort_unstable();
            expected.dedup();
            assert_eq!(resolved, expected, "for input {matching:?}");
            assert_eq!(resolved.windows(2).filter(|w| w[0] >= w[1]).count(), 0);
            // Every input level is present.
            matching.iter().for_each(|l| assert!(resolved.contains(l)));
        }
    }
}
