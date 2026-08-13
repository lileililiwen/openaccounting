use askama::Template;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "entities/list.html")]
pub struct EntityList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub entities: Vec<EntityRow>,
}

#[derive(Template)]
#[template(path = "entities/form.html")]
pub struct EntityForm {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub name: String,
    pub legal_name: String,
    pub tax_id: String,
    pub error: String,
}

#[derive(Template)]
#[template(path = "entities/consolidated.html")]
pub struct ConsolidatedBalanceSheet {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub entity_id: Uuid,
    pub entity_name: String,
    pub legal_name: String,
    pub ledger_rows: Vec<(String, Decimal, Decimal, Decimal)>,
    pub total_assets: Decimal,
    pub total_liabilities: Decimal,
    pub total_equity: Decimal,
    pub total_income: Decimal,
    pub total_expense: Decimal,
    pub net_income: Decimal,
    pub elimination_count: i64,
}

#[derive(Clone, Debug)]
pub struct EntityRow {
    pub id: Uuid,
    pub name: String,
    pub legal_name: String,
    pub tax_id: String,
    pub ledger_count: i64,
}
