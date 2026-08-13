pub mod balance_sheet;
pub mod cash_flow;
pub mod general_ledger;
pub mod income_statement;
pub mod trial_balance;

pub use balance_sheet::build_balance_sheet;
pub use cash_flow::build_cash_flow;
pub use general_ledger::{build_general_ledger, GeneralLedgerEntry};
pub use income_statement::build_income_statement;
pub use trial_balance::{build_trial_balance, TrialBalanceRow};

use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct AccountTotal {
    pub account_id: Uuid,
    pub account_name: String,
    pub account_type: String,
    pub amount: Decimal,
}
