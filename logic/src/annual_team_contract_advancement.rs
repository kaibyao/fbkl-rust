use color_eyre::Result;
use fbkl_entity::sea_orm::{ConnectionTrait, TransactionTrait};

/// Advances the contracts tied to teams in a league
pub async fn advance_team_contracts_for_league<C>(league_id: i64, db: &C) -> Result<()>
where
    C: ConnectionTrait + TransactionTrait,
{
    println!("Advancing contracts for league {}...", league_id);

    println!("{} contracts advanced in league {}", 0, league_id);
    Ok(())
}
