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
    pub current_section: String,
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
    pub current_section: String,
    pub account_types: Vec<crate::domain::AccountType>,
    pub error: String,
}

#[derive(Template)]
#[template(path = "accounts/edit.html")]
pub struct AccountEdit {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    pub account: Account,
    pub account_types: Vec<crate::domain::AccountType>,
    /// `(value, label)` subtype options valid for the account's current type.
    pub valid_subtypes: Vec<(String, String)>,
    /// False when the account has postings — type/subtype are locked.
    pub can_change_type: bool,
    pub error: String,
}

#[derive(Template)]
#[template(path = "accounts/opening_balances.html")]
pub struct OpeningBalancesPage {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub current_section: String,
    /// `(account, current balance)` for the balance-sheet accounts.
    pub accounts: Vec<(Account, Decimal)>,
    /// True when an opening-balances entry already exists.
    pub has_opening: bool,
    pub date: String,
    pub error: String,
}
