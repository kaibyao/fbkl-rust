//! Handles contract entity generation for signing RFA and UFA contracts to a team.

use std::cmp;

use crate::contract::{self, ContractKind};
use color_eyre::{
    Result,
    eyre::{bail, eyre},
};
use sea_orm::ActiveValue;

use super::ContractStatus;

/// The three contract kinds that can be signed as an extension; narrows the wide `ContractKind`
/// so the signing match is exhaustive without an unreachable arm.
enum FreeAgentKind {
    Restricted,
    UnrestrictedOriginalTeam,
    UnrestrictedVeteran,
}

impl FreeAgentKind {
    const fn from_contract_kind(kind: ContractKind) -> Option<Self> {
        match kind {
            ContractKind::RestrictedFreeAgent => Some(Self::Restricted),
            ContractKind::UnrestrictedFreeAgentOriginalTeam => Some(Self::UnrestrictedOriginalTeam),
            ContractKind::UnrestrictedFreeAgentVeteran => Some(Self::UnrestrictedVeteran),
            _ => None,
        }
    }
}

/// Creates a new Veteran or Rookie Extension contract from the given RFA or UFA contract as a result of a team winning the contract during the Preseason Veteran Auction.
pub fn sign_rfa_or_ufa_contract_to_team(
    fa_contract: &contract::Model,
    signing_team_id: i64,
    winning_bid_amount: i16,
) -> Result<contract::ActiveModel> {
    let fa_kind = FreeAgentKind::from_contract_kind(fa_contract.kind).ok_or_else(|| {
        eyre!(
            "Can only sign an RFA or UFA contract (given contract type: {:?}).",
            fa_contract.kind
        )
    })?;
    if fa_contract.status != ContractStatus::Active {
        bail!(
            "Cannot sign an extension for a replaced or expired contract. Contract:\n{:#?}",
            fa_contract
        );
    }

    let signing_original_team = fa_contract.team_id == Some(signing_team_id);
    let (new_contract_year, new_contract_type, new_salary) = match (signing_original_team, fa_kind)
    {
        (true, FreeAgentKind::Restricted) => (
            4,
            ContractKind::RookieExtension,
            // RFA 10% re-sign is uncapped, floored at the standard 4th-year salary the RFA contract already carries (rookie Y3 + 20%).
            cmp::max(
                discounted_salary(winning_bid_amount, 0.1, None),
                fa_contract.salary,
            ),
        ),
        (true, FreeAgentKind::UnrestrictedOriginalTeam) => (
            1,
            ContractKind::Veteran,
            discounted_salary(winning_bid_amount, 0.2, Some(8)),
        ),
        (true, FreeAgentKind::UnrestrictedVeteran) => (
            1,
            ContractKind::Veteran,
            discounted_salary(winning_bid_amount, 0.1, Some(5)),
        ),
        (false, _) => (1, ContractKind::Veteran, winning_bid_amount),
    };

    let new_contract = contract::ActiveModel {
        id: ActiveValue::NotSet,
        year_number: ActiveValue::Set(new_contract_year),
        kind: ActiveValue::Set(new_contract_type),
        is_ir: ActiveValue::Set(fa_contract.is_ir),
        salary: ActiveValue::Set(new_salary),
        end_of_season_year: ActiveValue::Set(fa_contract.end_of_season_year),
        status: ActiveValue::Set(fa_contract.status),
        league_id: ActiveValue::Set(fa_contract.league_id),
        league_player_id: ActiveValue::Set(fa_contract.league_player_id),
        player_id: ActiveValue::Set(fa_contract.player_id),
        previous_contract_id: ActiveValue::Set(Some(fa_contract.id)),
        original_contract_id: ActiveValue::Set(fa_contract.original_contract_id),
        team_id: ActiveValue::Set(Some(signing_team_id)),
        created_at: ActiveValue::NotSet,
        updated_at: ActiveValue::NotSet,
    };

    Ok(new_contract)
}

/// Discount `final_bid` by `rate` (rounded up), optionally capped at `max_discount` dollars, floored at $1.
// salaries are far below i16::MAX, so the rounded discount never truncates
#[allow(clippy::cast_possible_truncation)]
fn discounted_salary(final_bid: i16, rate: f32, max_discount: Option<i16>) -> i16 {
    let mut discount = (f32::from(final_bid) * rate).ceil() as i16;
    if let Some(cap) = max_discount {
        discount = discount.min(cap);
    }
    cmp::max(final_bid - discount, 1)
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use chrono::{DateTime, FixedOffset};
    use color_eyre::Result;
    use sea_orm::ActiveValue;

    use crate::contract::{
        ContractKind, ContractStatus, Model,
        free_agent_extension::{discounted_salary, sign_rfa_or_ufa_contract_to_team},
    };

    static NOW: LazyLock<DateTime<FixedOffset>> = LazyLock::new(|| {
        DateTime::parse_from_str("2023 Apr 13 12:09:14.274 +0000", "%Y %b %d %H:%M:%S%.3f %z")
            .unwrap()
    });

    fn generate_contract() -> Model {
        Model {
            id: 1,
            kind: ContractKind::RookieDevelopment,
            year_number: 1,
            salary: 4,
            is_ir: false,
            end_of_season_year: 2023,
            league_id: 1,
            league_player_id: None,
            player_id: Some(1),
            original_contract_id: Some(1),
            previous_contract_id: None,
            team_id: Some(1),
            status: ContractStatus::Active,
            created_at: NOW.to_owned(),
            updated_at: NOW.to_owned(),
        }
    }

    #[test]
    fn resign_rfa_same_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.kind = ContractKind::RestrictedFreeAgent;
        test_contract.salary = 4;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 1, 11)?;
        assert_eq!(advanced_contract.year_number, ActiveValue::Set(4));
        assert_eq!(
            advanced_contract.kind,
            ActiveValue::Set(ContractKind::RookieExtension)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(9));

        Ok(())
    }

    #[test]
    fn sign_rfa_different_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.kind = ContractKind::RestrictedFreeAgent;
        test_contract.salary = 4;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 2, 11)?;
        assert_eq!(advanced_contract.year_number, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.kind,
            ActiveValue::Set(ContractKind::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(11));

        Ok(())
    }

    #[test]
    fn sign_ufa20_same_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.kind = ContractKind::UnrestrictedFreeAgentOriginalTeam;
        test_contract.salary = 27;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 1, 33)?;
        assert_eq!(advanced_contract.year_number, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.kind,
            ActiveValue::Set(ContractKind::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(26));

        Ok(())
    }

    #[test]
    fn sign_ufa20_different_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.kind = ContractKind::UnrestrictedFreeAgentOriginalTeam;
        test_contract.salary = 27;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 2, 33)?;
        assert_eq!(advanced_contract.year_number, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.kind,
            ActiveValue::Set(ContractKind::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(33));

        Ok(())
    }

    #[test]
    fn sign_ufa10_same_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.kind = ContractKind::UnrestrictedFreeAgentVeteran;
        test_contract.salary = 27;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 1, 33)?;
        assert_eq!(advanced_contract.year_number, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.kind,
            ActiveValue::Set(ContractKind::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(29));

        Ok(())
    }

    #[test]
    fn sign_ufa10_new_team() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.kind = ContractKind::UnrestrictedFreeAgentVeteran;
        test_contract.salary = 27;

        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 2, 33)?;
        assert_eq!(advanced_contract.year_number, ActiveValue::Set(1));
        assert_eq!(
            advanced_contract.kind,
            ActiveValue::Set(ContractKind::Veteran)
        );
        assert_eq!(advanced_contract.salary, ActiveValue::Set(33));

        Ok(())
    }

    #[test]
    fn discounts_calculate_correctly() {
        // 10% uncapped (RFA path)
        assert_eq!(discounted_salary(1, 0.1, None), 1);
        assert_eq!(discounted_salary(2, 0.1, None), 1);
        assert_eq!(discounted_salary(3, 0.1, None), 2);
        assert_eq!(discounted_salary(10, 0.1, None), 9);
        assert_eq!(discounted_salary(11, 0.1, None), 9);
        assert_eq!(discounted_salary(30, 0.1, None), 27);
        assert_eq!(discounted_salary(31, 0.1, None), 27);

        // 20% uncapped
        assert_eq!(discounted_salary(1, 0.2, None), 1);
        assert_eq!(discounted_salary(5, 0.2, None), 4);
        assert_eq!(discounted_salary(10, 0.2, None), 8);
        assert_eq!(discounted_salary(40, 0.2, None), 32);
    }

    #[test]
    fn discount_caps_apply() {
        // 20% cap at $8: discount stops growing past $8 once bid exceeds $40.
        assert_eq!(discounted_salary(34, 0.2, Some(8)), 27);
        assert_eq!(discounted_salary(40, 0.2, Some(8)), 32);
        assert_eq!(discounted_salary(60, 0.2, Some(8)), 52);
        assert_eq!(discounted_salary(41, 0.2, Some(8)), 33);

        // 10% cap at $5: discount stops growing past $5 once bid exceeds $50.
        assert_eq!(discounted_salary(50, 0.1, Some(5)), 45);
        assert_eq!(discounted_salary(80, 0.1, Some(5)), 75);
        assert_eq!(discounted_salary(51, 0.1, Some(5)), 46);

        // RFA 10% is uncapped: high bids keep full 10% discount.
        assert_eq!(discounted_salary(80, 0.1, None), 72);
    }

    #[test]
    fn resign_rfa_floors_at_standard_salary() -> Result<()> {
        let mut test_contract = generate_contract();
        test_contract.kind = ContractKind::RestrictedFreeAgent;
        test_contract.salary = 20; // standard 4th-year salary the RFA carries

        // Low winning bid: 10% discount would drop to 9, floored at 20.
        let advanced_contract = sign_rfa_or_ufa_contract_to_team(&test_contract, 1, 11)?;
        assert_eq!(advanced_contract.salary, ActiveValue::Set(20));

        Ok(())
    }
}
