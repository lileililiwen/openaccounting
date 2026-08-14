//! Rule-based auto-match / auto-categorize for bank-statement
//! reconciliation.
//!
//! A rule has a `predicate` (the "when") and an `action` (the
//! "then"). Both are JSONB; the keys documented below are the
//! only ones the evaluator / applier knows about. Unknown keys
//! are ignored so a rule written today keeps working when new
//! keys land tomorrow.
//!
//! ## Predicate keys
//!
//! - `payee_glob`        — SQL `LIKE` pattern, matched against
//!   the bank-statement line's `description` (the existing
//!   column carries the payee for the tests in this repo).
//! - `description_glob`  — LIKE pattern against `description`.
//! - `amount_cents_eq` / `_lt` / `_gt` — integer cent amounts.
//!   (The schema stores `NUMERIC(20,4)`; we compare on the
//!   integer-cents projection to avoid `Decimal` rounding
//!   surprises.)
//! - `currency` — exact match.
//!
//! ## Action shapes
//!
//! - `categorize`: `{ "gl_account_id": "<uuid>" }`. The line's
//!   matched transaction gets its non-cash leg redirected to
//!   the named account.
//! - `match`: `{ "link_by_amount_date": true }`. The line is
//!   auto-paired with the closest posting of the same amount
//!   on the same date.
//! - `flag`: `{ "reason_text": "...", "flag_color": "amber" }`.
//!   Surfaces in the reconciliation page as a banner.

use serde_json::Value;
use uuid::Uuid;

/// One rule row, as stored in `reconciliation_rules`.
#[derive(Clone, Debug)]
pub struct Rule {
    pub id: Uuid,
    pub name: String,
    pub kind: RuleKind,
    pub priority: i32,
    pub predicate: Value,
    pub action: Value,
    pub is_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleKind {
    Match,
    Categorize,
    Flag,
}

impl RuleKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleKind::Match => "match",
            RuleKind::Categorize => "categorize",
            RuleKind::Flag => "flag",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "match" => Some(Self::Match),
            "categorize" => Some(Self::Categorize),
            "flag" => Some(Self::Flag),
            _ => None,
        }
    }
}

/// Evaluate the predicate against a `Line` (a bank-statement
/// line or any other record the engine can match). All
/// present keys are AND-combined; absence means "no
/// constraint".
pub fn evaluate_predicate(p: &Value, line: &Line) -> bool {
    if let Some(glob) = p.get("payee_glob").and_then(|v| v.as_str()) {
        if !sql_like_match(glob, &line.description) {
            return false;
        }
    }
    if let Some(glob) = p.get("description_glob").and_then(|v| v.as_str()) {
        if !sql_like_match(glob, &line.description) {
            return false;
        }
    }
    if let Some(eq) = p.get("amount_cents_eq").and_then(|v| v.as_i64()) {
        if line.amount_cents != eq {
            return false;
        }
    }
    if let Some(lt) = p.get("amount_cents_lt").and_then(|v| v.as_i64()) {
        if line.amount_cents >= lt {
            return false;
        }
    }
    if let Some(gt) = p.get("amount_cents_gt").and_then(|v| v.as_i64()) {
        if line.amount_cents <= gt {
            return false;
        }
    }
    if let Some(ccy) = p.get("currency").and_then(|v| v.as_str()) {
        if !line.currency.eq_ignore_ascii_case(ccy) {
            return false;
        }
    }
    true
}

#[derive(Clone, Debug, Default)]
pub struct Line {
    pub description: String,
    pub amount_cents: i64,
    pub currency: String,
}

/// Naive `LIKE` matcher: `%` matches any run, `_` matches one
/// character. Anything else is literal. Anchors are implicit.
pub fn sql_like_match(pattern: &str, s: &str) -> bool {
    // Translate to a regex.
    let mut regex = String::with_capacity(pattern.len() + 4);
    regex.push('^');
    for c in pattern.chars() {
        match c {
            '%' => regex.push_str(".*"),
            '_' => regex.push('.'),
            // Escape regex metacharacters.
            '.' | '(' | ')' | '[' | ']' | '\\' | '+' | '*' | '?' | '|' | '^' | '$' => {
                regex.push('\\');
                regex.push(c);
            }
            _ => regex.push(c),
        }
    }
    regex.push('$');
    regex::Regex::new(&regex)
        .map(|r| r.is_match(s))
        .unwrap_or(false)
}

/// Apply a list of rules to a single line. Returns the
/// first matching rule (lowest `priority`) along with the
/// decoded action. Inactive rules are ignored.
pub fn first_match<'a>(rules: &'a [Rule], line: &Line) -> Option<(&'a Rule, AppliedAction)> {
    let mut sorted: Vec<&Rule> = rules.iter().filter(|r| r.is_active).collect();
    sorted.sort_by_key(|r| r.priority);
    for r in sorted {
        if evaluate_predicate(&r.predicate, line) {
            return Some((r, AppliedAction::from(&r.action)));
        }
    }
    None
}

#[derive(Clone, Debug)]
pub enum AppliedAction {
    Categorize { gl_account_id: Uuid },
    Match { link_by_amount_date: bool },
    Flag { reason_text: String, flag_color: String },
}

impl AppliedAction {
    pub fn from(value: &Value) -> Self {
        if let Some(id) = value.get("gl_account_id").and_then(|v| v.as_str()) {
            if let Ok(uuid) = Uuid::parse_str(id) {
                return Self::Categorize { gl_account_id: uuid };
            }
        }
        if value
            .get("link_by_amount_date")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return Self::Match {
                link_by_amount_date: true,
            };
        }
        Self::Flag {
            reason_text: value
                .get("reason_text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            flag_color: value
                .get("flag_color")
                .and_then(|v| v.as_str())
                .unwrap_or("amber")
                .to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn matches_payee_glob() {
        let p = json!({"payee_glob": "STARBUCKS%"});
        let l = Line {
            description: "STARBUCKS #1234".into(),
            amount_cents: 4250,
            currency: "USD".into(),
        };
        assert!(evaluate_predicate(&p, &l));
    }

    #[test]
    fn rejects_non_matching_glob() {
        let p = json!({"payee_glob": "STARBUCKS%"});
        let l = Line {
            description: "AMAZON".into(),
            amount_cents: 1000,
            currency: "USD".into(),
        };
        assert!(!evaluate_predicate(&p, &l));
    }

    #[test]
    fn compound_predicates_are_anded() {
        let p = json!({
            "payee_glob": "STARBUCKS%",
            "amount_cents_eq": 4250
        });
        let ok = Line {
            description: "STARBUCKS #1".into(),
            amount_cents: 4250,
            currency: "USD".into(),
        };
        let wrong_amt = Line {
            description: "STARBUCKS #1".into(),
            amount_cents: 1000,
            currency: "USD".into(),
        };
        assert!(evaluate_predicate(&p, &ok));
        assert!(!evaluate_predicate(&p, &wrong_amt));
    }

    #[test]
    fn sql_like_handles_underscore() {
        assert!(sql_like_match("STARBUCKS_", "STARBUCKSA"));
        assert!(!sql_like_match("STARBUCKS_", "STARBUCKSAB"));
    }

    #[test]
    fn priority_tiebreaks_lowest_wins() {
        let r1 = Rule {
            id: Uuid::new_v4(),
            name: "high".into(),
            kind: RuleKind::Categorize,
            priority: 200,
            predicate: json!({"payee_glob": "A%"}),
            action: json!({"gl_account_id": Uuid::new_v4().to_string()}),
            is_active: true,
        };
        let r2 = Rule {
            id: Uuid::new_v4(),
            name: "low".into(),
            kind: RuleKind::Categorize,
            priority: 50,
            predicate: json!({"payee_glob": "A%"}),
            action: json!({"gl_account_id": Uuid::new_v4().to_string()}),
            is_active: true,
        };
        let rules = [r1, r2];
        let line = Line {
            description: "AMAZON".into(),
            amount_cents: 1000,
            currency: "USD".into(),
        };
        let (winner, _) = first_match(&rules, &line).unwrap();
        assert_eq!(winner.priority, 50);
    }

    #[test]
    fn inactive_rules_are_ignored() {
        let r = Rule {
            id: Uuid::new_v4(),
            name: "off".into(),
            kind: RuleKind::Categorize,
            priority: 1,
            predicate: json!({"payee_glob": "AMAZON%"}),
            action: json!({"gl_account_id": Uuid::new_v4().to_string()}),
            is_active: false,
        };
        let line = Line {
            description: "AMAZON".into(),
            amount_cents: 1000,
            currency: "USD".into(),
        };
        assert!(first_match(&[r], &line).is_none());
    }
}
