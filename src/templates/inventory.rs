use askama::Template;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "inventory/list.html")]
pub struct InventoryList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub items: Vec<InventoryItem>,
}

#[derive(Template)]
#[template(path = "inventory/form.html")]
pub struct InventoryForm {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub accounts: Vec<(Uuid, String, String)>,
    pub name: String,
    pub sku: String,
    pub description: String,
    pub error: String,
}

#[derive(Template)]
#[template(path = "inventory/valuation.html")]
pub struct InventoryValuation {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub items: Vec<InventoryItem>,
    pub total_value: Decimal,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct InventoryItem {
    pub id: Uuid,
    pub name: String,
    pub sku: String,
    pub description: String,
    pub asset_account_id: Uuid,
    pub cogs_account_id: Uuid,
    pub income_account_id: Uuid,
    pub quantity_on_hand: i32,
    pub unit_cost: Decimal,
    pub is_active: bool,
}
