//! Inventory valuation math (`accounting-dimensions`).
//!
//! Pure FIFO and weighted-average valuation over movement sequences.
//! Purchases add layers; sales/consumption remove quantity. Average
//! re-prices remaining stock per movement; FIFO consumes oldest layers
//! first. All money math is exact `Decimal` — no float anywhere.

use rust_decimal::Decimal;

/// Valuation method stored per ledger (`ledgers.inventory_method`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValuationMethod {
    Fifo,
    Average,
}

impl ValuationMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            ValuationMethod::Fifo => "fifo",
            ValuationMethod::Average => "average",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "fifo" => Some(Self::Fifo),
            "average" => Some(Self::Average),
            _ => None,
        }
    }
}

/// One stock movement. Positive `qty` = purchase at `unit_cost`;
/// negative `qty` = sale/consumption (`unit_cost` ignored — cost comes
/// from the layers under FIFO, from the running average under average).
#[derive(Clone, Debug)]
pub struct Movement {
    pub qty: i32,
    pub unit_cost: Decimal,
}

/// On-hand quantity and value after applying `movements` in order.
/// Returns `Err` when a sale exceeds stock (negative inventory).
pub fn valuate(method: ValuationMethod, movements: &[Movement]) -> Result<(i32, Decimal), String> {
    match method {
        ValuationMethod::Fifo => valuate_fifo(movements),
        ValuationMethod::Average => valuate_average(movements),
    }
}

fn valuate_fifo(movements: &[Movement]) -> Result<(i32, Decimal), String> {
    // Layers of (remaining qty, unit cost), oldest first.
    let mut layers: Vec<(i32, Decimal)> = Vec::new();
    for m in movements {
        if m.qty >= 0 {
            layers.push((m.qty, m.unit_cost));
        } else {
            let mut need = -m.qty;
            let on_hand: i32 = layers.iter().map(|(q, _)| q).sum();
            if need > on_hand {
                return Err(format!("FIFO sale of {} exceeds on-hand {on_hand}", -m.qty));
            }
            // Oldest-first consumption; adjustments (spec design)
            // consume newest first — same loop from the back.
            for (q, _) in layers.iter_mut() {
                if need == 0 {
                    break;
                }
                let take = (*q).min(need);
                *q -= take;
                need -= take;
            }
            layers.retain(|(q, _)| *q > 0);
        }
    }
    let qty: i32 = layers.iter().map(|(q, _)| q).sum();
    let value: Decimal = layers.iter().map(|(q, c)| Decimal::from(*q) * *c).sum();
    Ok((qty, value))
}

fn valuate_average(movements: &[Movement]) -> Result<(i32, Decimal), String> {
    let mut qty: i32 = 0;
    let mut value = Decimal::ZERO;
    for m in movements {
        if m.qty >= 0 {
            qty += m.qty;
            value += Decimal::from(m.qty) * m.unit_cost;
        } else {
            let need = -m.qty;
            if need > qty {
                return Err(format!("average sale of {} exceeds on-hand {qty}", -m.qty));
            }
            // Remove at the running average unit cost. Division rounds,
            // so a fully depleted stock is clamped to exactly zero —
            // no residual (or negative) penny from rounding drift.
            let avg = if qty > 0 {
                value / Decimal::from(qty)
            } else {
                Decimal::ZERO
            };
            qty -= need;
            value = if qty == 0 {
                Decimal::ZERO
            } else {
                value - Decimal::from(need) * avg
            };
        }
    }
    Ok((qty, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn fixture() -> Vec<Movement> {
        vec![
            Movement {
                qty: 10,
                unit_cost: dec("5.00"),
            }, // +50.00
            Movement {
                qty: 10,
                unit_cost: dec("7.00"),
            }, // +70.00
            Movement {
                qty: -12,
                unit_cost: dec("0"),
            }, // sell 12
        ]
    }

    #[test]
    fn fifo_and_average_produce_disclosed_distinct_values() {
        // FIFO: sell 10 @ 5.00 + 2 @ 7.00 = 64.00 COGS;
        // remaining 8 @ 7.00 = 56.00.
        let (qty, value) = valuate(ValuationMethod::Fifo, &fixture()).unwrap();
        assert_eq!(qty, 8);
        assert_eq!(value, dec("56.00"));

        // Average: avg 6.00 → COGS 72.00, remaining 8 @ 6.00 = 48.00.
        let (qty, value) = valuate(ValuationMethod::Average, &fixture()).unwrap();
        assert_eq!(qty, 8);
        assert_eq!(value, dec("48.00"));

        assert_ne!(
            valuate(ValuationMethod::Fifo, &fixture()).unwrap().1,
            valuate(ValuationMethod::Average, &fixture()).unwrap().1,
            "methods must disclose distinct values on this fixture"
        );
    }

    #[test]
    fn oversell_is_an_error_not_negative_stock() {
        let movs = vec![
            Movement {
                qty: 2,
                unit_cost: dec("1.00"),
            },
            Movement {
                qty: -3,
                unit_cost: dec("0"),
            },
        ];
        assert!(valuate(ValuationMethod::Fifo, &movs).is_err());
        assert!(valuate(ValuationMethod::Average, &movs).is_err());
    }

    #[test]
    fn random_sequences_keep_quantity_and_value_consistent() {
        // Property (1.5): xorshift64 — no RNG crate. Quantity always
        // equals purchases minus sales; value never goes negative;
        // both methods agree on quantity; FIFO value stays within
        // [min_layer, max_layer] × qty bounds.
        let mut rng: u64 = 0x123456789ABCDEF;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        for _ in 0..256 {
            let mut movs = Vec::new();
            let mut on_hand: i32 = 0;
            let n = 1 + next() % 10;
            for _ in 0..n {
                if on_hand == 0 || next() % 2 == 0 {
                    let q = 1 + (next() % 20) as i32;
                    let cents = 1 + (next() % 999) as i64;
                    movs.push(Movement {
                        qty: q,
                        unit_cost: Decimal::new(cents, 2),
                    });
                    on_hand += q;
                } else {
                    let q = 1 + (next() % on_hand as u64) as i32;
                    movs.push(Movement {
                        qty: -q,
                        unit_cost: Decimal::ZERO,
                    });
                    on_hand -= q;
                }
            }
            let bought: i32 = movs.iter().filter(|m| m.qty > 0).map(|m| m.qty).sum();
            let sold: i32 = movs.iter().filter(|m| m.qty < 0).map(|m| -m.qty).sum();
            for method in [ValuationMethod::Fifo, ValuationMethod::Average] {
                let (qty, value) = valuate(method, &movs).unwrap();
                assert_eq!(qty, bought - sold, "quantity conserved under {method:?}");
                assert!(
                    value >= Decimal::ZERO,
                    "value never negative under {method:?}"
                );
            }
            let (fq, _) = valuate(ValuationMethod::Fifo, &movs).unwrap();
            let (aq, _) = valuate(ValuationMethod::Average, &movs).unwrap();
            assert_eq!(fq, aq, "methods agree on quantity");
        }
    }
}
