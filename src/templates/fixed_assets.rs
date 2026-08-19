use askama::Template;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "fixed_assets/list.html")]
pub struct FixedAssetList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub assets: Vec<FixedAssetRow>,
}

#[derive(Template)]
#[template(path = "fixed_assets/form.html")]
pub struct FixedAssetForm {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub accounts: Vec<(Uuid, String, String)>,
    pub name: String,
    pub description: String,
    pub purchase_date: String,
    pub purchase_cost: String,
    pub salvage_value: String,
    pub useful_life_years: String,
    pub depreciation_method: String,
    pub error: String,
}

#[derive(Template)]
#[template(path = "fixed_assets/show.html")]
pub struct FixedAssetShow {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub asset: FixedAssetRow,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct FixedAssetRow {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub account_id: Uuid,
    pub purchase_date: NaiveDate,
    pub purchase_cost: Decimal,
    pub salvage_value: Decimal,
    pub useful_life_years: i32,
    pub depreciation_method: String,
    pub accumulated_depreciation: Decimal,
    pub status: String,
    pub disposed_date: Option<NaiveDate>,
    pub disposed_amount: Option<Decimal>,
}
