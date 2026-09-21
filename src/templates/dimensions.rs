use askama::Template;
use uuid::Uuid;

#[derive(Template)]
#[template(path = "dimensions/list.html")]
pub struct DimensionList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub cost_centers: Vec<DimensionRow>,
    pub projects: Vec<DimensionRow>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct DimensionRow {
    pub id: Uuid,
    pub name: String,
}
