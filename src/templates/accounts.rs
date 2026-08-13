use askama::Template;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::domain::Account;

#[derive(Template)]
#[template(path = "accounts/list.html")]
pub struct AccountList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub groups: Vec<AccountGroup>,
    pub balances: std::collections::HashMap<Uuid, Decimal>,
}

#[derive(Clone, Debug)]
pub struct AccountGroup {
    pub account_type: String,
    pub accounts: Vec<Account>,
}

#[derive(Template)]
#[template(path = "accounts/new.html")]
pub struct AccountNew {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub account_types: Vec<crate::domain::AccountType>,
    pub error: String,
}
