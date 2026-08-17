# Spec 03 — RFA Resolution & Draft-Pick Compensation

**Rules ref:** §14.4, §15, §16.4 · **Status:** 🔴 not built (discount math exists) · **Priority:** P1

## Summary

The discount **math** exists (`entity/src/entities/contract/free_agent_extension.rs::sign_rfa_or_ufa_contract_to_team`, caps fixed in [spec 04](04-ufa-rfa-discount-caps.md)). What is missing is the **process** around it:

1. **Designation** (§14.4, §15.4.2/§16.4): at the keeper deadline, players coming off R/3 → RFA and off V/3 or R/5 → UFA must be turned into `RestrictedFreeAgent` / `UnrestrictedFreeAgent*` contracts, and the **original owner** (team owning the contract at the keeper-deadline moment) must be snapshotted so the exception follows that team even if the player is later traded/sold during the auction.
2. **Resolution workflow** (§15.2.2, §15.3.2): after an RFA's auction closes, a three-stage timed handshake — winner has 48h to optionally raise → winner has 24h to name the pick he would forfeit → original owner has 48h to match-with-discount (re-sign) or decline. No-response defaults: no raise, the lowest-ranked eligible pick, no match. The pick is named before the owner decides because the owner needs to know what declining earns him.
3. **Compensation** (§15.2): if the original owner declines a *bid-on* RFA, the winning owner forfeits a Rookie-Draft pick (tier by final bid) to the original owner.

UFA designation has no resolution workflow (UFAs just enter the auction with the original owner allowed to bid + discount on win) — UFAs only need the designation half of this spec. The discount on a UFA/RFA *win or re-sign* is already handled in `sign_rfa_or_ufa_contract_to_team`.

`ContractKind` already has the target variants: `RestrictedFreeAgent` ("RFA"), `UnrestrictedFreeAgentOriginalTeam` ("UFA-OriginalTeam", 20% / 5-yr), `UnrestrictedFreeAgentVeteran` ("UFA-FreeAgent", 10% / 3-yr). Re-sign target `RookieExtension` (RFA) / `Veteran` (UFA) also exist.

## Backend

### RFA/UFA designation at keeper deadline (original-owner snapshot)

The keeper deadline (`DeadlineKind::PreseasonKeeper`, `TransactionKind::PreseasonKeeper`) is processed by `logic/src/deadline_processing/keeper_deadline/process_keeper_deadline.rs`. Today its inner loop (`process_keeper_deadline_transaction_inner`) handles only `ContractUpdateType::Keeper` (no-op) and `ContractUpdateType::Drop`, and `bail!`s on anything else. Non-kept, max-length contracts currently fall through with no designation path — that gap is what this spec fills.

Add a designation step that runs at keeper-deadline processing (after keepers/drops are applied), driven by contract eligibility rather than owner choice:

- `Rookie` year 4 (i.e. coming off R/3) → `RestrictedFreeAgent`.
- `Veteran` year 4 (coming off V/3) → `UnrestrictedFreeAgentVeteran` (3-yr, 10%).
- `RookieExtension` year 6 (coming off R/5) → `UnrestrictedFreeAgentOriginalTeam` (5-yr, 20%).

Use the season-year/contract-year boundaries from `constants/src/league_rules/config_settings.rs` rather than literals (cross-check max contract lengths there). Each designation creates a **new contract record** in the chain (convention #5: `previous_contract_id` set, `original_contract_id` carried) with the new `kind`, `status = Active`, `team_id` = the keeper-deadline owner. That `team_id` is the UFA "original owner" snapshot (§3.1.4, §16.4). RFAs cannot rely on it: a trade between the keeper deadline and the close of the auction rewrites `team_id`, while the discount stays with the keeper-deadline owner (§15.4.2), so the RFA exception holder is read from the **resolution** row (below). `sign_rfa_or_ufa_contract_to_team` therefore takes an explicit `FreeAgentException` from the caller that knows the rule instead of comparing team ids itself. A UFA traded mid-auction has the same gap and no resolution row to close it with, so it still reads its current `team_id`.

Designation is a transaction + team_update per convention #1. Reuse the `PreseasonKeeper` transaction or add a sibling; each designated contract is a `TeamUpdateAsset::Contracts` entry with a new `ContractUpdateType` (e.g. `RfaDesignation` / `UfaDesignation`), status `Done`.

### entity/ (RFA resolution state machine; raise/match deadlines; compensation pick record)

New table **`rfa_resolution`** (entity `entity/src/entities/rfa_resolution.rs` + queries `entity/src/queries/rfa_resolution_queries.rs`):

```
id                        i64 pk
league_id                 i64
end_of_season_year        i16
rfa_contract_id           i64   -- the RestrictedFreeAgent contract being resolved
original_owner_team_id    i64   -- snapshot from keeper deadline (§15.4.2)
auction_id                i64?  -- null = not bid on (§15.3.5 no-bid path)
winning_team_id           i64?  -- null until/unless bid on
final_bid                 i16?  -- final winning bid; null if no-bid
final_bid_at              DateTimeWithTimeZone?  -- timestamp the winning-bid email/event was sent (§15.2.2), drives compensation eligibility
status                    RfaResolutionStatus
raised_bid                 i16?  -- winner's optional raise (>= final_bid), §15.3.2.1
raise_deadline_at          DateTimeWithTimeZone?  -- auction_close + 48h; null until the auction closes
pick_selection_deadline_at DateTimeWithTimeZone?  -- set when the raise stage resolves; +24h
match_deadline_at          DateTimeWithTimeZone?  -- set when the pick selection resolves; +48h
resolved_at               DateTimeWithTimeZone?
created_at / updated_at
```

`RfaResolutionStatus` (sea_orm string-value enum, mirror `ContractKind`/`TransactionKind` style):

```
AwaitingAuction       -- designated at the keeper deadline; the player's auction has not closed yet
AwaitingRaise         -- auction closed; winner's 48h raise window open
AwaitingPickSelection -- raise resolved (raised or declined/timed-out); winner's 24h pick-selection window open
AwaitingMatch         -- pick named (chosen or auto-selected); original owner's 48h window open
Resolved        -- original owner matched → re-signed at discount (RookieExtension)
Declined        -- original owner declined → winner signs at final bid + forfeits pick
NoBidResigned   -- §15.3.5: not bid on, original owner re-signed at 4th-yr 10% discount
NoBidToAuction  -- §15.3.5: not bid on, original owner declined → new Veteran contract in regular auction
```

New table **`rfa_compensation_pick`** (links the forfeited pick to the resolution; the actual pick reassignment reuses trade-style transfer — see [spec 07](07-pick-transfer.md)):

```
id                       i64 pk
rfa_resolution_id        i64
required_round            i16   -- tier-derived round (§15.2.1)
forfeited_draft_pick_id   i64?  -- chosen by winning owner from eligible set (§15.2.2)
to_team_id                i64   -- original owner
from_team_id              i64   -- winning owner
created_at / updated_at
```

`draft_pick` already carries `current_owner_team_id` / `original_owner_team_id` / `round` / `end_of_season_year` — sufficient to enumerate eligible picks. No schema change to `draft_pick` needed.

`rfa_compensation_pick` is written when the winner names his pick, which is before the original owner decides. A match therefore leaves the row behind with `forfeited_draft_pick_id` set and the pick never moving — the record of a debt that did not come due.

Add `TransactionKind` variants (string-valued): `RfaRaiseBid`, `RfaResign`, `RfaDeclineAndForfeit`. Each resolution mutation records one.

### logic/ (`logic/src/deadline_processing/rfa_resolution/` — new module)

Follow conventions: each step = transaction + team_update; wrap multi-step mutations in `db.begin()…commit()` (#2); delegate persistence to `entity/src/queries/` (#3); validate before mutating (#4).

- **`designate_rfas_ufas(league_id, end_of_season_year, db)`** — invoked from keeper-deadline processing. Finds active contracts at max length, creates designation contracts (above), and for each RFA inserts an `rfa_resolution` row seeded with `original_owner_team_id`. (UFAs need no resolution row.)

- **`raise_bid(rfa_resolution_id, raising_team_id, new_bid, db)`** — guard: resolution `status == AwaitingRaise`, caller `== winning_team_id`, `new_bid > final_bid`, and per §15.3.3 the would-be compensation pick must be one the winner actually holds (reject a raise that pushes into a tier the winner can't pay — call `compute_eligible_compensation_picks` with the *raised* bid and `ensure!` non-empty). Sets `raised_bid`, transitions `AwaitingRaise → AwaitingPickSelection`, sets `pick_selection_deadline_at = now + RFA_PICK_SELECTION_WINDOW_HOURS`. Transaction `RfaRaiseBid` + team_update (winner; cap impact, status `Pending`). A no-raise (explicit decline or scheduler timeout) also transitions to `AwaitingPickSelection` without setting `raised_bid`.

- **`select_compensation_pick(rfa_resolution_id, selecting_team_id, draft_pick_id, now, db)`** — guard: `status == AwaitingPickSelection`, caller `== winning_team_id`, and the pick is in `compute_eligible_compensation_picks`. Writes `rfa_compensation_pick` (`required_round` from the effective bid, `forfeited_draft_pick_id` = the choice) and transitions to `AwaitingMatch` with `match_deadline_at = now + 48h`. Naming the pick early ends the window, the same way standing pat ends the raise window.

- **`expire_pick_selection_window(rfa_resolution_id, now, db)`** — the scheduler's timeout. Names the eligible pick that costs the winner least, then opens the match window the same way. "Costs least" = latest in rookie-draft order (§7.2.1): the season's `rookie_draft_selection` slate when the draft has started, else the order recomputed from the lottery draw and the frozen standings (`logic::rookie_draft::find_season_draft_pick_order`). A season whose lottery has not been drawn has no intra-round order at all, so the highest round number stands in and a warning is logged.

- **`match_or_decline(rfa_resolution_id, original_owner_team_id, decision, now, db)`** — guard: `status == AwaitingMatch`, caller `== original_owner_team_id`. `AwaitingMatch` is only reachable through a settled pick selection, so the pick is already named by the time this runs.
  - **Match** → call `sign_rfa_or_ufa_contract_to_team(rfa_contract, original_owner_team_id, effective_bid)` where `effective_bid = raised_bid.unwrap_or(final_bid)` (discount + caps handled there, [spec 04](04-ufa-rfa-discount-caps.md)); produces `RookieExtension` year 4. Status → `Resolved`. Transaction `RfaResign` + team_update. The winner's cap-hold (§15.3.4) is released.
  - **Decline** → winner signs at `effective_bid` (`sign_rfa_or_ufa_contract_to_team(rfa_contract, winning_team_id, effective_bid)` → `Veteran` year 1, no discount since `team_id != winning_team_id`). Read the pick the winner already named off `rfa_compensation_pick` and check he still holds it; reassign it via trade-style transfer ([spec 07](07-pick-transfer.md)). Status → `Declined`. Transaction `RfaDeclineAndForfeit` + team_updates for both teams (winner: −pick +contract; original owner: +pick).

- **No-bid path (§15.3.5)** — `resolve_unbid_rfa(rfa_resolution_id, decision, db)`: if `auction_id.is_none()`. Re-sign → `sign_rfa_or_ufa_contract_to_team(.., original_owner_team_id, standard_4th_yr_salary)` at 10% discount off the standard 4th-yr salary (status `NoBidResigned`); decline → designation contract flips to a fresh `FreeAgent`/`Veteran` path for the regular Veteran Auction (status `NoBidToAuction`). No compensation pick (only bid-on declines forfeit picks).

- **`compute_eligible_compensation_picks(rfa_resolution, winning_team_id, db) -> Vec<draft_pick::Model>`**:
  1. `final = rfa_resolution.raised_bid.unwrap_or(final_bid)`.
  2. `required_round = compensation_round_for_bid(final)` from the constants table (below).
  3. Candidate picks = `draft_pick` where `league_id` matches, `end_of_season_year == upcoming Rookie Draft year`, `current_owner_team_id == winning_team_id`, and `round <= required_round` ("or better" = an earlier/lower round number is acceptable; §15.2.2).
  4. **Exclude picks acquired after the winning bid** (§15.2.2 worked example): drop any candidate whose ownership transferred to `winning_team_id` *after* `rfa_resolution.final_bid_at`. Determine acquisition time from the latest `trade`/`trade_asset` transfer of that `draft_pick` to `winning_team_id` (timestamp = trade-announcement time; see open questions). A pick the winner held *before* the bid, or that was never traded, is eligible.
  5. **Exclude picks another handshake already named**: drop any candidate an `AwaitingMatch` resolution's `rfa_compensation_pick` row points at. That promise is still outstanding, and one pick cannot settle two debts. A declined resolution has already moved its pick out of the winner's hands; a matched one owes nothing.
  6. If the result is empty for the exact round but earlier-round picks exist they are already included by `round <= required_round`; if multiple remain, the winner chooses (UI). Return the set (caller persists the chosen one).

  `compensation_round_for_bid(final_bid) -> i16` lives in logic but reads the tier table from `constants/`.

### constants/ (the bid→round compensation tier table)

Add to `constants/src/league_rules/` (e.g. `rfa_compensation.rs`), `///`-documented per the crate convention, the §15.2.1 tiers (round = "or better", so the value is the *highest* round number acceptable):

| final bid | required round (or better) |
|-----------|----------------------------|
| ≤ $11     | 5 |
| $12–$18   | 4 |
| $19–$27   | 3 |
| $28–$41   | 2 |
| ≥ $42     | 1 |

Expose as an ordered `[(max_bid_inclusive, round)]` slice + a lookup fn so logic doesn't duplicate literals (logic/CLAUDE.md "Where rule values live").

### scheduler (48h window expiry defaults — cross-ref [spec 05](05-scheduler.md))

All three windows need scheduled expiry jobs, driven off `find_rfa_resolutions_with_expired_window` and dispatched as `ProcessableEventKind::RfaRaiseWindowExpiry` / `RfaPickSelectionExpiry` / `RfaMatchWindowExpiry`:
- At `raise_deadline_at`: if still `AwaitingRaise`, auto-transition to `AwaitingPickSelection` (no raise) and set `pick_selection_deadline_at`.
- At `pick_selection_deadline_at`: if still `AwaitingPickSelection`, auto-select the lowest-ranked eligible pick, write `rfa_compensation_pick`, and open the match window.
- At `match_deadline_at`: if still `AwaitingMatch`, auto-`Declined` (no match) and run the decline/forfeit path against the already-named pick.

Window lengths (48h / 24h / 48h) are defaults that belong in constants, not hardcoded in logic: `RFA_RAISE_WINDOW_HOURS`, `RFA_PICK_SELECTION_WINDOW_HOURS`, `RFA_MATCH_WINDOW_HOURS`. Cross-ref [spec 05](05-scheduler.md).

### GraphQL (cross-ref [spec 06](06-graphql.md))

Expose: query `rfaResolutions(leagueId, endOfSeasonYear)` returning resolution state + all three countdown deadlines + (for the acting team) the eligible-pick set; mutations `raiseRfaBid`, `declineToRaiseRfa`, `selectRfaCompensationPick(rfaResolutionId, draftPickId)` (winner only), `matchRfa`, `declineRfa`, `resignUnbidRfa`, `releaseUnbidRfaToAuction`. Resolvers delegate straight to the logic fns above (server/ holds no logic). See [spec 06](06-graphql.md).

## Frontend (Next.js + MUI v7 + urql)

- **Winner raise UI**: for resolutions in `AwaitingRaise` owned-as-winner, show the player, final bid, a 48h countdown (derive from `raise_deadline_at`), and a raise input (`raiseRfaBid`) with a "decline to raise" action. Surface the projected compensation tier for the current/raised bid (computed backend-side, not re-derived in JS — [spec 04](04-ufa-rfa-discount-caps.md) note).
- **Original-owner match/decline UI**: for `AwaitingMatch` where current user is `original_owner_team_id`, show effective bid, the backend-computed re-sign salary (after 10% discount + caps), match-deadline countdown, and Match / Decline actions (`matchRfa` / `declineRfa`).
- **Compensation-pick selector**: for resolutions in `AwaitingPickSelection` owned-as-winner, present the `compute_eligible_compensation_picks` set as a chooser with a 24h countdown; the winning owner submits `selectRfaCompensationPick` (per §15.2.2 the *winner* picks among eligible picks). Say what the timeout would take.
- **No-bid panel**: for `auction_id == null` resolutions owned by the original owner, offer re-sign (`resignUnbidRfa`) vs. release-to-auction.

## Edge cases & open questions

- **No-bid path**: a designated RFA never bid on (§15.3.5) skips the raise/match handshake entirely → only re-sign-at-discount or release-to-Veteran-Auction. Ensure `designate_rfas_ufas` does not create the `AwaitingRaise` state for these; the resolution row should start in a no-bid state (or `auction_id` stays null and the scheduler skips it).
- **Ranking before the lottery**: auto-select needs rookie-draft order to say which eligible pick is cheapest. §7.2.2 runs the lottery shortly after the prior regular season, so the order should exist by the preseason auction — but nothing enforces that ordering today. Until it does, a league whose lottery has not been drawn falls back to the highest round number.
- **Cap-hold during resolution (§15.3.4)**: between auction close and resolution the winning owner counts as the winning bidder for cap purposes in *other* auctions. Need to confirm the auction/cap engine ([spec 01](01-live-auction-engine.md)) reads in-flight `rfa_resolution.winning_team_id` + `effective_bid` as a committed cap obligation. Open: does releasing the hold on Match correctly free the winner's cap mid-auction?
- **Timestamp source for "acquired after bid" (§15.2.2)**: rules define winning-bid time = the email that *ended up* winning (not the 24h auction close, not the raise decision), and trade time = announcement email (not confirmation). We need a reliable `final_bid_at` from the auction engine and a per-`draft_pick` acquisition timestamp from trade history. Confirm `trade`/`trade_asset` records carry an announcement timestamp distinct from confirmation; if not, that's a dependency on [spec 01](01-live-auction-engine.md)/[spec 07](07-pick-transfer.md).
- **Raise that creates an unpayable compensation obligation (§15.3.3)**: an owner may not raise into a tier requiring a pick he doesn't hold. `raise_bid` must reject; confirm this is also enforced at *original bid* time in the auction engine ([spec 01](01-live-auction-engine.md)), not only at raise.
- **RFA re-sign max length (§15.3.6)**: re-signed RFA → max 5-yr (RookieExtension, years 4–5) — already the existing `RookieExtension` path; verify advancement (`annual_contract_advancement`) expires it to UFA-20 after year 5.

## Dependencies

- [spec 01](01-live-auction-engine.md) — auction close event, `final_bid_at`, in-flight cap holds.
- [spec 04](04-ufa-rfa-discount-caps.md) — discount caps in `sign_rfa_or_ufa_contract_to_team` (must land first; this spec calls it).
- [spec 05](05-scheduler.md) — 48h raise/match window expiry jobs.
- [spec 06](06-graphql.md) — resolver/mutation surface.
- [spec 07](07-pick-transfer.md) — trade-style draft-pick reassignment for the forfeited compensation pick.
