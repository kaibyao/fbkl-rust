# Spec 07 - Trade Legality, Deadline & Picks

**Rules ref:** §12 · **Status:** 🟡 core trades + T1/T2 legality work; deadline gate, pick window, auction guard and conditional picks missing · **Priority:** P1

Revised 2026-09-11 after the d1r transaction model (#143) and a review of epic fbkl-rust-8zs.

## Summary

Propose/accept/process is built (`logic/src/trade/`): `propose_trade`, `accept_trade`, `process_trade`,
`reject_trade`, multi-owner + one-way trades, `external_trade_invalidation`, and accommodating drops
carried by the proposer and each accepter (spec 08). `accept_trade` processes once every recorded
`trade_action` is `Propose`/`Accept` and every involved team has responded. **Do not respec that.**

Trade legality is also built. One trade is one rules §13.1.4.1 transaction, so `process_trade`
applies the legs plus each side's accommodating moves and then runs `file_and_validate_transaction`
per involved team (T1 roster legality including cap, T2 no same-transaction add-then-remove,
`logic/src/trade/process_trade.rs`). A trade that leaves a team illegal is rejected with
`TradeLeavesRostersIllegal`, which the resolver maps through `roster_move_error`. An earlier draft
of this spec proposed a warn-only legality report; rules §13.1.6 T1 rules that out.

This spec adds the missing gates and the conditional-pick resolution:

1. **Trade deadline** (§12.3) - `DeadlineKind::TradeDeadlineAndPlayoffStart` exists but nothing
   blocks proposing/accepting after it. Epic child fbkl-rust-8zs.1.
2. **Pick tradability window** (§12.4) - picks tradable up to 2 years out; "next year" begins when
   the Rookie Draft concludes. Nothing enforces the window. fbkl-rust-8zs.2.
3. **Auction guard** - a contract that an unresolved auction references can be traded, which leaves
   `auction.contract_id` on a `Replaced` row. fbkl-rust-8zs.3.
4. **Conditional trades** (§12.5.1) - `draft_pick_option` and its statuses exist, but nothing
   creates an option, nothing evaluates the condition, and `Used` is never set. fbkl-rust-8zs.6/.7.

Done since the first draft: the `.expect()` panic on a missing pre-trade salary is gone
(`MissingPreTradeSalary`, `logic/src/trade/create_trade_team_update.rs`, commit ad8232b).

Out of scope (cross-ref [spec 12](12-out-of-scope-and-external.md)): §12.5.3 multi-part / players-to-be-named-later
(not allowed), §12.5.4 rentals (not allowed), §12.6 collusion (commissioner discretion, no auto-veto).

## Backend

### Rejection policy

Every check below is a hard rejection with a typed error, mapped in `map_trade_processing_error`
(`server/src/graphql/trade/trade_resolvers.rs`). A bare `eyre!` error falls through
`roster_move_error` and reaches the client as `ErrorCode::Internal` with no message. The full set
of trade rejections after this spec: T1/T2 (built), the deadline gate, the pick window, the
auction guard, and an invalid conditional clause.

**Import replay.** The historical import calls `propose_trade` and `accept_trade` for every CSV
trade (`import-data/src/league/league_events/seasonal_trade_league_events.rs`).
`TradeLegality::CallerJudges` skips T1/T2 only; every new gate runs during replay with no bypass.
Each child carries the fresh-league replay recipe (`bd memories fresh-league`) as an acceptance gate. Add
a bypass only if the replay shows a historical trade the gate refuses.

### Trade-time legality (built)

`validate_trade_assets` (`logic/src/trade/validate_trade_assets.rs`) checks contract
latest-in-chain + owned by `from_team`; draft pick owned by `from_team`; option `Proposed`. It runs
from `process_trade` only. Roster and cap legality run after it, per team, through
`validate_team_roster` (`logic/src/deadline_processing/roster_lock/validate_rosters.rs`, pub) via
`logic/src/roster/transaction.rs`. Tests: `server/tests/trade_accommodating_drops.rs`,
`server/tests/trade_add_joins_lock_week.rs`.

If a pre-accept advisory preview is wanted for the frontend, it is a new issue. It must project each
team's whole transaction (legs plus that team's accommodating drops and IR moves, which
`process_trade` applies after the asset snapshot), not the asset-only salary snapshot.

### Trade deadline gate (`DeadlineKind::TradeDeadlineAndPlayoffStart`)

§12.3: deadline = roster lock the first week of the playoffs; no trades until after the playoffs
(`SeasonEnd`). Nothing enforces this today.

- Add `validate_trade_window_open(league_id, end_of_season_year, action_datetime, db)` in
  `logic/src/trade/`. Look up `TradeDeadlineAndPlayoffStart` and `SeasonEnd` for the season with
  `deadline_queries::find_deadline_for_season_by_type` and compare `action_datetime` against their
  `date_time` directly: closed iff `TradeDeadlineAndPlayoffStart <= action_datetime < SeasonEnd`.
  Do not use `find_most_recent_deadline_by_datetime`: `InSeasonRosterLock` deadlines continue
  through the playoff weeks (`server/tests/playoff_week_roster_moves.rs`), so the most recent
  deadline inside the closed window is a lock.
- `propose_trade` takes no datetime today; add `propose_datetime`. Callers: the GraphQL resolver,
  the historical import (pass `args.trade_datetime`), and the test callers in `server/tests/` and
  `jobs/tests/`. `accept_trade` already takes `accept_datetime`.
- Call the check in **both** `propose_trade` and `accept_trade`, because a trade proposed before
  the deadline must not be accepted after it.
- Known limit, own issue: `process_trade` needs an upcoming roster lock in the trade's season
  (`find_upcoming_roster_lock`, season-scoped). Between `SeasonEnd` and the next season's first lock
  none exists, so an accept there fails with `MissingUpcomingRosterLock` even though §12.3 reopens
  trades, and the resolver still selects the ended season. Offseason trade filing is not this spec.

### Pick tradability window (§12.4 two-year rule; window advances after the Rookie Draft)

§12.4: picks tradable up to two years out; the "next year" begins immediately after the Rookie
Draft concludes. `draft_pick.end_of_season_year` is the discriminator; `FUTURE_DRAFT_PICK_SEASONS_LIMIT = 2`.

- Extend `validate_draft_pick_trade_asset` (`validate_trade_assets.rs`, ownership only today).
  `pick.end_of_season_year` must be `> N` and `<= N + FUTURE_DRAFT_PICK_SEASONS_LIMIT`, where N is
  the latest concluded draft year. Rules examples: before and during the 2010 draft N = 2009; after
  it concludes N = 2010.
- **Draft conclusion has no schema marker.** `DeadlineKind` has `PreseasonRookieDraftStart` only and
  `rookie_draft_selection` has no completion timestamp. Define it per pick: the draft for season Y
  has concluded iff every `draft_pick` row for (league, Y) has a `rookie_draft_selection` row and
  none of them is `Unused`. A pick with no selection, or any `Unused` selection, means not concluded.
  The per-pick form matters because `start_rookie_draft` inserts the whole slate as `Unused` up
  front while the historical import inserts one selection per replayed pick or pass, so a
  rows-exist-and-none-`Unused` test would read as concluded after the first replayed pick. For a
  trade in season Y: N = Y if concluded(Y), else Y - 1. Do not infer conclusion from the start
  deadline having passed; §7.3.3 and §12.4.1 allow trades during the draft.
- **Consumed picks.** A selection becomes `PlayerSelected` or `Skipped` while the `draft_pick` row
  stays. Reject a pick whose selection is no longer `Unused`; the year window alone lets a used pick
  trade during the draft.
- **Where it runs.** `validate_trade_assets` is called from `process_trade` only. Call the pick check
  from `propose_trade` with `propose_datetime` as well, then again at process (eligibility can
  change between the two).
- Apply the same check to every pick linked to a `draft_pick_option` asset through
  `draft_pick_draft_pick_option` (`draft_pick_queries::get_draft_picks_affected_by_options`), not to
  the one related pick `new_trade_asset_active_model_by_id` reads today.
- Prerequisite gap: `generate_future_draft_picks` is never called live (its only call site is
  commented out in `roster_lock/lock_rosters.rs`), so in a live league the N+2 picks do not exist.
  fbkl-rust-e1q.

### Auction guard (fbkl-rust-8zs.3, absorbs fbkl-rust-tox)

A trade replaces the contract row, so `auction.contract_id` then points at a `Replaced` row. The RFA
decline path reads that stale row (`logic/src/auction/preseason_veteran_auction.rs`); the match path
walks the chain. Reject a trade of a contract with an auction in `Pending`, `Open`, `Closed` or
`Won`. `Closed` is included on purpose: RFA auctions park there for the raise/match window.
`Completed` and `Expired` do not block. Add an `auction_queries` lookup by contract id; none exists.

### Conditional draft-pick trades (`draft_pick_option`)

The option lifecycle today: `process_trade` flips `Proposed` -> `Active` (`process_trade_assets`);
external invalidation sets `InvalidatedByExternalTrade`. Three gaps: **no code creates an option**
(`propose_trade` inserts `trade_asset` rows only; the GraphQL input takes the id of an option that
must already exist), `reject_trade` never writes `CancelledViaTradeRejection`, and nothing evaluates
the condition or sets `Used`.

- **Condition representation** (fbkl-rust-8zs.6): `clause` is a free-text `String`. Serialize a
  structured, position-only condition into it as JSON:
  `{ source_draft_pick_id, position_range: [lo, hi] inclusive, then_pick_ids, else_pick_ids }`,
  plus a rendered string for display. Per §12.5.1 only draft-pick position conditions are allowed;
  reject anything else at propose time.
- **Position coordinate:** one-based overall draft order, the value `rookie_draft_selection.order`
  stores. `draft_pick` has a round but no position. Both §12.5.1 examples condition on a first-round
  pick, where overall order equals in-round position; a later-round source uses overall order too.
  Validate `lo <= hi` within 1..=slate size.
- **Pick links:** `draft_pick_draft_pick_option` lists every pick an option can affect (source, then
  set, else set) so existing queries keep working; the JSON assigns roles. Write both in one
  database transaction. The two §12.5.1 examples transfer different pick bundles per range, so
  then/else are pick *sets*.
- **Creation path:** the `proposeTrade` input carries the clause; `propose_trade` inserts the option
  (`Proposed`), its junction rows and the `DraftPickOption` `trade_asset` atomically, after
  validating that the source and every then/else pick exist, belong to the league, are owned by the
  option's from team, and pass the pick window.
- **Beneficiary:** `draft_pick_option` has no owner column. The beneficiary is the `to_team_id` of
  the `trade_asset` that carried the option in its completed trade. Re-trading the source pick later
  does not change it; trading an `Active` option is already rejected.
- **Rejection:** `reject_trade` sets the trade's options to `CancelledViaTradeRejection`.
- **Audit fix:** `create_trade_team_update.rs` zips the flattened pick list against the option list,
  so a multi-pick option yields one `DraftPickOptionAdded` row and mispairs the next option. Group
  picks by option id and write one row per affected pick.
- **Resolution** (fbkl-rust-8zs.7): `resolve_draft_pick_options` in `logic/src/rookie_draft/`,
  idempotent, walking `Active` options whose source pick has a selection row. Triggers: (a)
  `start_rookie_draft` after it persists the slate ([spec 02](02-rookie-draft-engine.md)); (b)
  `process_trade` when an option becomes `Active` and the slate already exists (§7.3.3 allows
  trades during the draft). Steps:
  1. position = `rookie_draft_selection.order` of the source pick; in range -> then set, else ->
     else set.
  2. For each pick in the chosen set, set `draft_pick.current_owner_team_id` to the beneficiary
     (mirror `update_trade_asset_draft_pick`) **and** the pick's `Unused`
     `rookie_draft_selection.current_owner_team_id`. `make_pick` awards the player to the selection's
     owner and the draft resolver authorizes against it, so updating `draft_pick` alone gives the pick
     to the wrong team.
  3. Set the option `Used`; insert a `league_event` (the table formerly named `transaction`) with a
     new `LeagueEventKind` variant, plus a `team_update` per affected team with a new
     `DraftPickUpdateType` variant next to `DraftPickOptionAdded`. This event is neither
     `team_update.transaction_number` (the §13 weekly transaction) nor the SQL transaction. Wrap in
     `db.begin()`/`commit()` per `logic/CLAUDE.md`.

## Frontend (React/Vite + TanStack Router + shadcn/Base UI + Tailwind + urql)

The GraphQL trade API exists (`proposeTrade`, `acceptTrade`, `rejectTrade` in
`server/src/graphql/trade/`). New fields this spec adds (structured clause input, typed rejection
codes) go on that API. Frontend work is deferred and is not a child of fbkl-rust-8zs.

- **Trade builder UI**: multi-team asset picker (contracts + picks + conditional options), reflecting
  one-way and multi-owner trades, with the accommodating-drop input spec 08 defines. Asset lists
  filtered to assets the `from_team` owns (latest-in-chain contracts, in-window unused picks only).
- **Rejection display**: render the typed rejection (T1/T2 violations, deadline, pick window,
  auction guard, clause) as the rule message the resolver returns.
- **Conditional-pick condition editor**: position-range builder only (e.g. "picks 1-3 -> bundle A,
  else bundle B"). No player/team performance inputs (§12.5.1).
- **Deadline-closed state**: when the trade window is closed (between `TradeDeadlineAndPlayoffStart`
  and `SeasonEnd`), disable propose/accept actions and show why.

## Edge cases & open questions

- **Offseason trade filing**: see the known limit under the deadline gate. Needs a decision on which
  lock an offseason trade files under and how the resolver picks the season; §4.2.4 keeps the $230
  cap until contract advancement runs, so the period must not resolve as uncapped.
- **Source pick re-traded before resolution**: the beneficiary is fixed by the option's trade asset,
  so the resolution transfers to that team whoever owns the source pick at the time.
- **Conditional picks and live RFA obligations (spec 03 interplay)**: RFA compensation is enforced by
  bid-time naming (`rfa_compensation_pick`, `find_reserved_compensation_pick_ids`), not by an
  acquired-after timestamp. A pick in a then/else set must not double as a named compensation pick;
  coordinate with fbkl-rust-abp, which adds the reservation check to trade validation.
- **Window boundary precision**: settled above; conclusion = every pick has a selection and none is `Unused`. If the league
  ever wants a commissioner-declared end instead, add a completion instant to the slate.

## Dependencies

- [spec 02](02-rookie-draft-engine.md) - `start_rookie_draft` persists the slate that resolves conditional picks.
- [spec 03](03-rfa-resolution-and-compensation.md) - named compensation picks (`rfa_compensation_pick`) vs picks inside conditional options.
- [spec 05](05-deadline-scheduler-and-transaction-processor.md) - season deadlines the trade-deadline gate reads.
- [spec 06](06-graphql-api-surface.md) - trade queries/mutations (built); new clause input and rejection codes extend them.
- [spec 08](08-weekly-moves-and-roster-legalization.md) - the transaction model (T1/T2, accommodating drops) that trade processing already runs.
