use color_eyre::{Result, eyre::eyre};
use fbkl_entity::{
    deadline::{self, DeadlineKind, DeadlineStatus},
    deadline_config_rule,
    sea_orm::{ActiveModelTrait, ConnectionTrait, Set, prelude::DateTimeWithTimeZone},
};
use std::fmt::Debug;
use tracing::instrument;

/// Generated deadline with calculated datetime and metadata
#[derive(Debug, Clone)]
pub struct GeneratedDeadline {
    pub kind: DeadlineKind,
    pub date_time: DateTimeWithTimeZone,
    pub name: String,
    pub status: DeadlineStatus,
}

/// Generate deadline records from configuration rules
#[instrument]
pub fn generate_deadlines_from_config(
    config: &deadline_config_rule::Model,
) -> Result<Vec<GeneratedDeadline>> {
    let mut deadlines = Vec::new();

    // 1. Preseason Keeper deadline (absolute datetime from config)
    deadlines.push(GeneratedDeadline {
        kind: DeadlineKind::PreseasonKeeper,
        date_time: config.preseason_keeper_deadline,
        name: format!("Preseason Keeper Deadline - {}", config.end_of_season_year),
        status: DeadlineStatus::Draft,
    });

    // 2. Veteran Auction Start (days after keeper deadline, at 9am)
    let veteran_auction_start = config.preseason_keeper_deadline
        + chrono::Duration::days(config.veteran_auction_days_after_keeper_deadline_duration as i64);

    // Set to 9am on the calculated date
    let veteran_auction_start_9am = veteran_auction_start
        .date_naive()
        .and_hms_opt(9, 0, 0)
        .ok_or_else(|| eyre!("Invalid datetime calculation for veteran auction start"))?
        .and_utc()
        .into();

    deadlines.push(GeneratedDeadline {
        kind: DeadlineKind::PreseasonVeteranAuctionStart,
        date_time: veteran_auction_start_9am,
        name: format!(
            "Preseason Veteran Auction Start - {}",
            config.end_of_season_year
        ),
        status: DeadlineStatus::Draft,
    });

    // 3. Rookie Draft Start (3 days after veteran auction, at 9am - per PRD)
    let rookie_draft_start = veteran_auction_start_9am + chrono::Duration::days(3);
    let rookie_draft_start_9am = rookie_draft_start
        .date_naive()
        .and_hms_opt(9, 0, 0)
        .ok_or_else(|| eyre!("Invalid datetime calculation for rookie draft start"))?
        .and_utc()
        .into();

    deadlines.push(GeneratedDeadline {
        kind: DeadlineKind::PreseasonRookieDraftStart,
        date_time: rookie_draft_start_9am,
        name: format!(
            "Preseason Rookie Draft Start - {}",
            config.end_of_season_year
        ),
        status: DeadlineStatus::Draft,
    });

    // 4. FA Auction Start (3 hours after veteran auction starts - per PRD)
    let fa_auction_start = veteran_auction_start_9am + chrono::Duration::hours(3);
    deadlines.push(GeneratedDeadline {
        kind: DeadlineKind::PreseasonFaAuctionStart,
        date_time: fa_auction_start,
        name: format!("Preseason FA Auction Start - {}", config.end_of_season_year),
        status: DeadlineStatus::Draft,
    });

    // 5. FA Auction End (FA auction duration after FA auction start)
    let fa_auction_end =
        fa_auction_start + chrono::Duration::days(config.fa_auction_days_duration as i64);
    deadlines.push(GeneratedDeadline {
        kind: DeadlineKind::PreseasonFaAuctionEnd,
        date_time: fa_auction_end,
        name: format!("Preseason FA Auction End - {}", config.end_of_season_year),
        status: DeadlineStatus::Draft,
    });

    // 6. Final Roster Lock (days after rookie draft ends)
    let final_roster_lock = rookie_draft_start_9am
        + chrono::Duration::days(config.final_roster_lock_deadline_days_after_rookie_draft as i64);
    deadlines.push(GeneratedDeadline {
        kind: DeadlineKind::PreseasonFinalRosterLock,
        date_time: final_roster_lock,
        name: format!(
            "Preseason Final Roster Lock - {}",
            config.end_of_season_year
        ),
        status: DeadlineStatus::Draft,
    });

    // 7. Week 1 FA Auction Start (starts when final roster lock is processed - per PRD)
    // For now, we'll set it to the same time as final roster lock, but this will be
    // dynamically created by the processing engine
    deadlines.push(GeneratedDeadline {
        kind: DeadlineKind::Week1FreeAgentAuctionStart,
        date_time: final_roster_lock,
        name: format!(
            "Week 1 Free Agent Auction Start - {}",
            config.end_of_season_year
        ),
        status: DeadlineStatus::Draft,
    });

    // Note: Trade deadline and playoff start will use the playoffs_start_week
    // but requires NBA schedule data to calculate exact datetime
    // This will be handled by a separate function when schedule data is available

    Ok(deadlines)
}

/// Save generated deadlines to the database
#[instrument]
pub async fn save_generated_deadlines<C>(
    deadlines: Vec<GeneratedDeadline>,
    league_id: i64,
    end_of_season_year: i16,
    db: &C,
) -> Result<Vec<deadline::Model>>
where
    C: ConnectionTrait + Debug,
{
    let mut saved_deadlines = Vec::new();

    for generated_deadline in deadlines {
        let deadline_model = deadline::ActiveModel {
            date_time: Set(generated_deadline.date_time),
            kind: Set(generated_deadline.kind),
            name: Set(generated_deadline.name),
            end_of_season_year: Set(end_of_season_year),
            league_id: Set(league_id),
            status: Set(generated_deadline.status),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
            ..Default::default()
        };

        let saved_deadline = deadline_model.insert(db).await?;
        saved_deadlines.push(saved_deadline);
    }

    Ok(saved_deadlines)
}

/// Calculate dependency order for deadline processing
#[instrument]
pub fn get_deadline_dependency_order() -> Vec<DeadlineKind> {
    // Order based on PRD requirements - each deadline depends on the previous being processed
    vec![
        DeadlineKind::PreseasonKeeper,
        DeadlineKind::PreseasonVeteranAuctionStart,
        DeadlineKind::PreseasonFaAuctionStart,
        DeadlineKind::PreseasonRookieDraftStart,
        DeadlineKind::PreseasonFaAuctionEnd,
        DeadlineKind::PreseasonFinalRosterLock,
        DeadlineKind::Week1FreeAgentAuctionStart,
        // Season deadlines will be added later when in-season processing is implemented
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn test_generate_deadlines_from_config() {
        let config = deadline_config_rule::Model {
            id: 1,
            league_id: 1,
            end_of_season_year: 2025,
            preseason_keeper_deadline: (Utc::now() + Duration::days(30)).into(),
            veteran_auction_days_after_keeper_deadline_duration: 7,
            fa_auction_days_duration: 5,
            final_roster_lock_deadline_days_after_rookie_draft: 5,
            playoffs_start_week: 21,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        };

        let deadlines = generate_deadlines_from_config(&config).unwrap();

        // Should generate 7 deadlines
        assert_eq!(deadlines.len(), 7);

        // Verify deadline types are included
        let kinds: Vec<DeadlineKind> = deadlines.iter().map(|d| d.kind).collect();
        assert!(kinds.contains(&DeadlineKind::PreseasonKeeper));
        assert!(kinds.contains(&DeadlineKind::PreseasonVeteranAuctionStart));
        assert!(kinds.contains(&DeadlineKind::PreseasonRookieDraftStart));
        assert!(kinds.contains(&DeadlineKind::PreseasonFaAuctionStart));
        assert!(kinds.contains(&DeadlineKind::PreseasonFaAuctionEnd));
        assert!(kinds.contains(&DeadlineKind::PreseasonFinalRosterLock));
        assert!(kinds.contains(&DeadlineKind::Week1FreeAgentAuctionStart));

        // All deadlines should start in Draft status
        assert!(deadlines.iter().all(|d| d.status == DeadlineStatus::Draft));
    }

    #[test]
    fn test_deadline_timing_calculations() {
        let keeper_date = Utc::now() + Duration::days(30);
        let config = deadline_config_rule::Model {
            id: 1,
            league_id: 1,
            end_of_season_year: 2025,
            preseason_keeper_deadline: keeper_date.into(),
            veteran_auction_days_after_keeper_deadline_duration: 7,
            fa_auction_days_duration: 5,
            final_roster_lock_deadline_days_after_rookie_draft: 5,
            playoffs_start_week: 21,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        };

        let deadlines = generate_deadlines_from_config(&config).unwrap();

        // Find specific deadlines to test timing
        let _keeper_deadline = deadlines
            .iter()
            .find(|d| d.kind == DeadlineKind::PreseasonKeeper)
            .unwrap();
        let veteran_auction = deadlines
            .iter()
            .find(|d| d.kind == DeadlineKind::PreseasonVeteranAuctionStart)
            .unwrap();
        let rookie_draft = deadlines
            .iter()
            .find(|d| d.kind == DeadlineKind::PreseasonRookieDraftStart)
            .unwrap();

        // Veteran auction should be 7 days after keeper
        let expected_veteran_start = keeper_date + Duration::days(7);
        assert_eq!(
            veteran_auction.date_time.date_naive(),
            expected_veteran_start.date_naive()
        );

        // Rookie draft should be 3 days after veteran auction
        let expected_rookie_start = expected_veteran_start + Duration::days(3);
        assert_eq!(
            rookie_draft.date_time.date_naive(),
            expected_rookie_start.date_naive()
        );
    }

    #[test]
    fn test_dependency_order() {
        let order = get_deadline_dependency_order();

        // Should have all preseason deadlines in correct order
        assert_eq!(order[0], DeadlineKind::PreseasonKeeper);
        assert_eq!(order[1], DeadlineKind::PreseasonVeteranAuctionStart);
        assert_eq!(order[2], DeadlineKind::PreseasonFaAuctionStart);
        assert_eq!(order[3], DeadlineKind::PreseasonRookieDraftStart);

        // Ensure no duplicates - simple check for this test
        assert!(!order.is_empty());
        // Manual duplicate check for the key deadlines
        let keeper_count = order
            .iter()
            .filter(|&&x| x == DeadlineKind::PreseasonKeeper)
            .count();
        assert_eq!(keeper_count, 1);
    }
}
