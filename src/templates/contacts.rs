use askama::Template;
use uuid::Uuid;

use crate::domain::Contact;

#[derive(Template)]
#[template(path = "contacts/list.html")]
pub struct ContactList {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub contacts: Vec<Contact>,
}

#[derive(Template)]
#[template(path = "contacts/new.html")]
pub struct ContactNew {
    pub user_id: Uuid,
    pub username: String,
    pub user_role: String,
    pub ledger_id: Uuid,
    pub ledger_name: String,
    pub error: String,
}
