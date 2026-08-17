# Spec 03 — RFA Resolution & Draft-Pick Compensation

**Rules ref:** §14.4, §15, §16.4 · **Status:** 🔴 not built (discount math exists) · **Priority:** P1

## Summary

The discount **math** exists (`entity/src/entities/contract/free_agent_extension.rs::sign_rfa_or_ufa_contract_to_team`, caps fixed in [spec 04](04-ufa-rfa-discount-caps.md)). What is missing is the **process** around it:

1. **Designation** (§14.4, §15.4.2/§16.4): at the keeper deadline, players coming off R/3 → RFA and off V/3 or R/5 → UFA must be turned into `RestrictedFreeAgent` / `UnrestrictedFreeAgent*` contracts, and the **original owner** (team owning the contract at the keeper-deadline moment) must be snapshotted so the exception follows that team even if the player is later traded/sold during the auction.
2. **Resolution workflow** (§15.2.2, §15.3.2): after an RFA's auction closes, a two-stage timed handshake — winner has 48h to optionally raise → original owner has 48h to match-with-discount (re-sign) or decline. No-response defaults: no raise, no match. Nothing has to be chosen in between, because the pick a decline would cost is named by the bid itself (below).
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
final_bid_at              DateTimeWithTimeZone?  -- timestamp the winning-bid email/event was sent (§15.2.2)
status                    RfaResolutionStatus
raised_bid                 i16?  -- winner's optional raise (>= final_bid), §15.3.2.1
raise_deadline_at          DateTimeWithTimeZone?  -- auction_close + 48h; null until the auction closes
match_deadline_at          DateTimeWithTimeZone?  -- set when the raise stage resolves; +48h
resolved_at               DateTimeWithTimeZone?
created_at / updated_at
```

`RfaResolutionStatus` (sea_orm string-value enum, mirror `ContractKind`/`TransactionKind` style):

```
AwaitingAuction       -- designated at the keeper deadline; the player's auction has not closed yet
AwaitingRaise         -- auction closed; winner's 48h raise window open
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
forfeited_draft_pick_id   i64   -- named by the bidder from the eligible set (§15.2.2, §15.3.3)
to_team_id                i64   -- original owner
from_team_id              i64   -- the team currently leading the bid
created_at / updated_at
```

`draft_pick` already carries `current_owner_team_id` / `original_owner_team_id` / `round` / `end_of_season_year` — sufficient to enumerate eligible picks. No schema change to `draft_pick` needed.

`rfa_compensation_pick` is written by the **first bid** on the RFA and rewritten by every later bid, raise or swap, so at any moment it says what the team currently leading would forfeit. One row per resolution (unique index on `rfa_resolution_id`), which is why being outbid frees the previous leader's pick with no bookkeeping of its own. A match leaves the row behind with the pick never moving — the record of a debt that did not come due.

Add `TransactionKind` variants (string-valued): `RfaRaiseBid`, `RfaResign`, `RfaDeclineAndForfeit`. Each resolution mutation records one.

### logic/ (`logic/src/deadline_processing/rfa_resolution/` — new module)

Follow conventions: each step = transaction + team_update; wrap multi-step mutations in `db.begin()…commit()` (#2); delegate persistence to `entity/src/queries/` (#3); validate before mutating (#4).

- **`designate_rfas_ufas(league_id, end_of_season_year, db)`** — invoked from keeper-deadline processing. Finds active contracts at max length, creates designation contracts (above), and for each RFA inserts an `rfa_resolution` row seeded with `original_owner_team_id`. (UFAs need no resolution row.)

- **bid time (`logic::auction::place_auction_bid`)** — a bid on a contract whose resolution is `AwaitingAuction` must carry `maybe_compensation_draft_pick_id`, and that pick must be in `eligible_compensation_picks` for the bid's tier. Rejections are `BidRejection::MissingCompensationPick` (none named) and `BidRejection::IneligibleCompensationPick` (named one cannot pay, or the auction owes nothing). A valid bid writes `rfa_compensation_pick` inside the bid's own transaction. This is where §15.3.3 is enforced; a released RFA (`NoBidToAuction`) is a plain free agent again, so the gate keys off resolution status, not contract kind.

- **`raise_bid(rfa_resolution_id, raising_team_id, new_bid, compensation_draft_pick_id, now, db)`** — guard: resolution `status == AwaitingRaise`, caller `== winning_team_id`, `new_bid > final_bid`, and the named pick must settle the *raised* tier (§15.3.3). Rewrites `rfa_compensation_pick`, sets `raised_bid`, transitions `AwaitingRaise → AwaitingMatch` with `match_deadline_at = now + RFA_MATCH_WINDOW_HOURS`. Transaction `RfaRaiseBid` + team_update (winner; cap impact, status `Pending`). A no-raise (explicit decline or scheduler timeout) also transitions to `AwaitingMatch` without setting `raised_bid`, leaving the pick the winning bid named.

- **`change_compensation_pick(rfa_resolution_id, naming_team_id, draft_pick_id, db)`** — the winner swaps his named pick for another that settles the same tier (§15.2.2 gives him the choice). Allowed while `status` is `AwaitingAuction` (his bid still leads) or `AwaitingRaise`; refused from `AwaitingMatch` on, because the original owner is by then deciding against a named pick. Without it a choice made at bid time could block a later bid the team's remaining picks could otherwise cover.

- **`match_or_decline(rfa_resolution_id, original_owner_team_id, decision, now, db)`** — guard: `status == AwaitingMatch`, caller `== original_owner_team_id`. The winning bid named the pick, so it is already there when this runs.
  - **Match** → call `sign_rfa_or_ufa_contract_to_team(rfa_contract, original_owner_team_id, effective_bid)` where `effective_bid = raised_bid.unwrap_or(final_bid)` (discount + caps handled there, [spec 04](04-ufa-rfa-discount-caps.md)); produces `RookieExtension` year 4. Status → `Resolved`. Transaction `RfaResign` + team_update. The winner's cap-hold (§15.3.4) is released.
  - **Decline** → winner signs at `effective_bid` (`sign_rfa_or_ufa_contract_to_team(rfa_contract, winning_team_id, effective_bid)` → `Veteran` year 1, no discount since `team_id != winning_team_id`). Read the pick the winner already named off `rfa_compensation_pick` and check he still holds it; reassign it via trade-style transfer ([spec 07](07-pick-transfer.md)). Status → `Declined`. Transaction `RfaDeclineAndForfeit` + team_updates for both teams (winner: −pick +contract; original owner: +pick).

- **No-bid path (§15.3.5)** — `resolve_unbid_rfa(rfa_resolution_id, decision, db)`: if `auction_id.is_none()`. Re-sign → `sign_rfa_or_ufa_contract_to_team(.., original_owner_team_id, standard_4th_yr_salary)` at 10% discount off the standard 4th-yr salary (status `NoBidResigned`); decline → designation contract flips to a fresh `FreeAgent`/`Veteran` path for the regular Veteran Auction (status `NoBidToAuction`). No compensation pick (only bid-on declines forfeit picks).

- **`eligible_compensation_picks(league_id, end_of_season_year, team_id, required_round, excluded_rfa_resolution_id, db) -> Vec<draft_pick::Model>`**:
  1. `required_round = compensation_round_for_bid(bid_amount)`, from the constants table (below); callers pass the round because they need it for their own error messages.
  2. Candidate picks = `draft_pick` where `league_id` matches, `end_of_season_year == upcoming Rookie Draft year`, `current_owner_team_id == team_id`, and `round <= required_round` ("or better" = an earlier/lower round number is acceptable; §15.2.2).
  3. **Exclude picks the team has already named elsewhere**: drop any candidate pointed at by an `rfa_compensation_pick` row whose resolution is `AwaitingAuction`, `AwaitingRaise` or `AwaitingMatch` and whose `from_team_id` is this team. One pick cannot settle two debts. `excluded_rfa_resolution_id` is the debt being priced, whose own row is about to be rewritten.
  4. Return the set, best round first. Empty means the bid is one §15.3.3 refuses.

  **§15.2.2's "acquired after the winning bid" clause needs no code.** A bid can only name a pick its bidder holds at the moment it is placed, so a pick acquired later was never nameable by that bid. That is why `final_bid_at` no longer drives eligibility, and why no trade-history lookup is needed.

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

Both windows need scheduled expiry jobs, driven off `find_rfa_resolutions_with_expired_window` and dispatched as `ProcessableEventKind::RfaRaiseWindowExpiry` / `RfaMatchWindowExpiry`:
- At `raise_deadline_at`: if still `AwaitingRaise`, auto-transition to `AwaitingMatch` (no raise) and set `match_deadline_at`.
- At `match_deadline_at`: if still `AwaitingMatch`, auto-`Declined` (no match) and run the decline/forfeit path against the pick the winning bid named.

Window lengths (48h / 48h) are defaults that belong in constants, not hardcoded in logic: `RFA_RAISE_WINDOW_HOURS`, `RFA_MATCH_WINDOW_HOURS`. Cross-ref [spec 05](05-scheduler.md).

### GraphQL (cross-ref [spec 06](06-graphql.md))

Expose: query `rfaResolutions(leagueId, endOfSeasonYear)` returning resolution state, both countdown deadlines, the named `compensationPick`, and `swappableCompensationPicks` (what the winner may swap to during the raise period); query `eligibleCompensationPicks(auctionId, bidAmount)` for the bid form, empty for an auction that owes nothing; mutations `placeBid(auctionId, bidAmount, compensationDraftPickId, comment)`, `raiseRfaBid(rfaResolutionId, bidAmount, compensationDraftPickId)`, `declineToRaiseRfa`, `changeRfaCompensationPick(rfaResolutionId, draftPickId)` (leading bidder only), `matchRfa`, `declineRfa`, `resignUnbidRfa`, `releaseUnbidRfaToAuction`. Resolvers delegate straight to the logic fns above (server/ holds no logic). See [spec 06](06-graphql.md).

## Frontend (Next.js + MUI v7 + urql)

- **Winner raise UI**: for resolutions in `AwaitingRaise` owned-as-winner, show the player, final bid, a 48h countdown (derive from `raise_deadline_at`), and a raise input (`raiseRfaBid`) with a "decline to raise" action. Surface the projected compensation tier for the current/raised bid (computed backend-side, not re-derived in JS — [spec 04](04-ufa-rfa-discount-caps.md) note).
- **Original-owner match/decline UI**: for `AwaitingMatch` where current user is `original_owner_team_id`, show effective bid, the backend-computed re-sign salary (after 10% discount + caps), match-deadline countdown, and Match / Decline actions (`matchRfa` / `declineRfa`).
- **Compensation-pick selector on the bid form**: a bid on an RFA needs a pick chooser fed by `eligibleCompensationPicks(auctionId, bidAmount)`, re-read as the amount changes because the tier moves with it. The same chooser backs `changeRfaCompensationPick` while the bid leads or the raise period is open.
- **No-bid panel**: for `auction_id == null` resolutions owned by the original owner, offer re-sign (`resignUnbidRfa`) vs. release-to-auction.

## Edge cases & open questions

- **No-bid path**: a designated RFA never bid on (§15.3.5) skips the raise/match handshake entirely → only re-sign-at-discount or release-to-Veteran-Auction. Ensure `designate_rfas_ufas` does not create the `AwaitingRaise` state for these; the resolution row should start in a no-bid state (or `auction_id` stays null and the scheduler skips it).
- **Cap-hold during resolution (§15.3.4)**: between auction close and resolution the winning owner counts as the winning bidder for cap purposes in *other* auctions. Need to confirm the auction/cap engine ([spec 01](01-live-auction-engine.md)) reads in-flight `rfa_resolution.winning_team_id` + `effective_bid` as a committed cap obligation. Open: does releasing the hold on Match correctly free the winner's cap mid-auction?
- **Naming the pick at bid time departs from the rules text.** §15.3.2.2 says the compensatory pick "is specified by the winning owner after original owner declines". Naming it with the bid instead is what makes §15.3.3 checkable and §15.2.2's acquired-after-bid clause automatic, and it costs the winner nothing but the timing of a choice whose tier the bid already fixed. Needs commissioner sign-off.
- **A named pick must not leave the team while the debt is live.** `match_or_decline` re-checks ownership before forfeiting, but nothing yet stops a trade from moving a named pick out. That guard belongs with pick transfers ([spec 07](07-pick-transfer.md)).
- **RFA re-sign max length (§15.3.6)**: re-signed RFA → max 5-yr (RookieExtension, years 4–5) — already the existing `RookieExtension` path; verify advancement (`annual_contract_advancement`) expires it to UFA-20 after year 5.

## Dependencies

- [spec 01](01-live-auction-engine.md) — auction close event, `final_bid_at`, in-flight cap holds.
- [spec 04](04-ufa-rfa-discount-caps.md) — discount caps in `sign_rfa_or_ufa_contract_to_team` (must land first; this spec calls it).
- [spec 05](05-scheduler.md) — 48h raise/match window expiry jobs.
- [spec 06](06-graphql.md) — resolver/mutation surface.
- [spec 07](07-pick-transfer.md) — trade-style draft-pick reassignment for the forfeited compensation pick.
