pub mod amortization;
pub mod balance_sheet;
pub mod cash_flow;
pub mod cash_flow_forecast;
pub mod general_ledger;
pub mod holdings;
pub mod income_statement;
pub mod inter_entity;
pub mod realized_gains;
pub mod trial_balance;

pub use balance_sheet::{build_balance_sheet, BalanceSheetSection};
pub use cash_flow::build_cash_flow;
pub use cash_flow_forecast::{
    advance, build_forecast, materialize_forecast, project, ForecastEntry, ForecastPoint,
    ForecastResult,
};
pub use general_ledger::{build_general_ledger, GeneralLedgerEntry};
pub use income_statement::{build_income_statement, IncomeStatementSection};
pub use trial_balance::{build_trial_balance, TrialBalanceRow};

use crate::error::{AppError, AppResult};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct AccountTotal {
    pub account_id: Uuid,
    pub account_name: String,
    pub account_type: String,
    pub amount: Decimal,
}

/// The reporting basis a request is computed under.
///
/// `Accrual` (the default) sums every income / expense posting
/// in the period regardless of whether cash has actually moved.
/// `Cash` is restricted to postings whose peer leg is a
/// cash or bank account — i.e. revenue is only counted when
/// cash is received, expense only when cash is paid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportBasis {
    Accrual,
    Cash,
}

impl Default for ReportBasis {
    fn default() -> Self {
        Self::Accrual
    }
}

impl ReportBasis {
    /// The canonical database string. Matches the CHECK
    /// constraint added by migration 0020.
    pub fn as_str(&self) -> &'static str {
        match self {
            ReportBasis::Accrual => "accrual",
            ReportBasis::Cash => "cash",
        }
    }

    /// Title-cased display label for the report header and
    /// basis toggle button.
    pub fn label(&self) -> &'static str {
        match self {
            ReportBasis::Accrual => "Accrual",
            ReportBasis::Cash => "Cash",
        }
    }

    /// Parse a `?basis=…` query value (or the stored ledger
    /// string). Empty string defaults to accrual, matching the
    /// repository's default.
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "accrual" | "" => Ok(Self::Accrual),
            "cash" => Ok(Self::Cash),
            other => Err(AppError::Validation(format!(
                "Unknown basis '{other}', expected 'accrual' or 'cash'."
            ))),
        }
    }
}
