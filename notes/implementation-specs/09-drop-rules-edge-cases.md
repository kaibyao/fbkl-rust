# Spec 09 — Drop Rules Edge Cases
**Rules ref:** §9 · **Status:** 🟡 core drops work; death-waiver + min-bid timing missing · **Priority:** P2

## Summary

Core drop flow is implemented and correct: `logic/src/drop_contract/drop_contract_from_team.rs`
drops an Active contract, records a `TeamUpdateDropContract` transaction (storing
`dropped_contract_id`) + a `Drop` team_update (Done), and captures pre/post salary+cap. Penalty
math lives in `logic/src/roster/salary_calculation.rs::calculate_team_contract_salary`:
per-contract `ceil(salary * 0.2)` (§9.1.1–.2, not combined), subtracted from max cap, recomputed
each call from `find_contracts_dropped_by_team_in_regular_season` so it never carries into next
season (§9.1.3). RD/RDI/RFA/UFA-exception kinds drop to $1/Expired and are excluded from the
penalty fold via `CONTRACT_TYPES_COUNTED_TOWARD_CAP` (only Rookie/RookieExtension/Veteran), so
they are penalty-free (§9.1.5, §11.8.1).

This spec covers the small remaining gaps: (1) the **Nick Adenhart rule** (§9.1.4) — no
penalty-waiver path exists; (2) the **dropped-player min-bid data contract** (§9.2.1) — where the
retained salary is read from and when it resets; (3) **released-week-after timing** (§9.2.2) — when
a dropped player becomes biddable; and verifying the (4) §9.1.1 after-keeper-deadline scoping.

## Backend

### Nick Adenhart rule (deceased → penalty-free drop): PlayerStatus/deceased flag, penalty-waived param on `drop_contract_from_team`, commissioner authz

Today there is **no override path** — every Active Rookie/RookieExtension/Veteran drop is folded
into the penalty in `calculate_team_contract_salary` (`salary_calculation.rs:93-99`), and the
penalty is derived purely from `find_contracts_dropped_by_team_in_regular_season`, so simply
dropping a deceased player still incurs the 20%.

1. **Deceased marker.** `entity/src/entities/player.rs::PlayerStatus` currently has only
   `{ Retired, Active }`. Add a `Deceased` variant (`#[sea_orm(string_value = "Deceased")]`) —
   migration in `migration/`. Note `league_player` (custom non-NBA players) has no analogous
   status today; decide whether the death-waiver applies to league_players (probably yes — scope
   the marker so both `player` and `league_player`-backed contracts can be flagged, or carry the
   waiver purely on the transaction as in step 3 to avoid needing a league_player status column).

2. **Penalty-waived flag on the dropped contract / transaction.** The penalty fold reads dropped
   *contracts*, not players, so the waiver must be discoverable at fold time without re-joining to
   player status (player could be un-flagged later). Add a boolean column
   `is_penalty_waived` (default false) to the **transaction** row (it already carries
   `dropped_contract_id` + `TransactionKind::TeamUpdateDropContract`) OR to the dropped contract
   record. Then exclude waived drops in `find_contracts_dropped_by_team_in_regular_season` (or in
   the `salary_calculation.rs` fold) so `ceil(salary*0.2)` is never charged. Prefer the
   transaction column — it keeps the audit trail explicit and avoids polluting the contract chain.

3. **`drop_contract_from_team` param.** Add `penalty_waived: bool` (or a
   `DropReason { Standard, Deceased }` enum) threaded into
   `drop_contract_from_team` → `create_drop_contract_team_update` →
   `transaction::ActiveModel.is_penalty_waived`. Keep `create_dropped_contract`'s salary/status
   logic unchanged — the retained-salary min-bid (§9.2.1) still applies to a deceased player's drop;
   only the *penalty* is waived, not the min-bid retention.

4. **Commissioner authz.** A penalty-waived drop must require commissioner authorization (the
   rules let *the owner* drop, but only the commissioner can certify the death + waive the
   penalty). Enforce at the GraphQL resolver layer (cross-ref spec 06) by checking
   `LeagueRole`/commissioner role before allowing `penalty_waived = true`; the logic fn stays
   role-agnostic. Record who authorized it for audit (commissioner user_id on the transaction).

### Dropped-player min-bid data contract (where retained salary read from for FA min bid; season-end reset)

§9.2.1: a dropped player retains his pre-drop salary as the **minimum opening bid** in in-season
FA for the rest of the season; after season end the minimum clears. Ground truth from
`entity/src/entities/contract/drop_contract.rs::create_dropped_contract`:

- The dropped player gets a **new `ContractKind::FreeAgent`** record (year 1,
  `previous_contract_id` = the dropped contract).
- For in-season Active drops of `Rookie`/`RookieExtension`/`Veteran`, that FreeAgent record's
  `salary` is set to the **pre-drop salary** (`new_salary_for_active_players_after_drop`,
  status `Active`). For RD/RDI/RFA/UFA-exception kinds (and *any* pre-keeper-deadline drop) it is
  reset to `$1`, status `Expired`.

**Data contract for the auction engine (spec 01):** the FA min opening bid = the `salary` on the
latest-in-chain `FreeAgent` contract for that player/league/season, **when that FreeAgent contract
is still `Active`**. This already matches §8.3.3 ("previous salary" min for previously-owned
players, RD/RDI included). The auction engine must read the min from this contract record, not
recompute it. Note the apparent gap vs §11.8.1: rules say dropped *RD/RDI* players also retain
their salary as min bid, but `create_dropped_contract` forces RD/RDI dropped salary to `$1`.
Flag and verify: either the rules intent is "$1 RD min anyway" or this is a bug — confirm against
intended behavior before spec 01 relies on it.

**Season-end reset:** there is no explicit "clear min bid" step needed — the FreeAgent contract is
expired by annual advancement (`annual_contract_advancement/`, PreseasonStart expires FAs per
`logic/CLAUDE.md`). Once expired, it is no longer latest-Active, so the min-bid lookup returns
nothing and the player re-enters the Veteran Auction / Rookie Draft pool with no minimum (§9.2.1).
Spec 01 must therefore scope the min-bid query to `status == Active` FreeAgent contracts only.
Verify `annual_contract_advancement` actually expires these dropped FreeAgent records.

### Released-week-after timing (when dropped player becomes biddable — cross-ref specs 01, 05)

§9.2.2: a dropped player enters the FA pool **the week after** he is dropped, not immediately.
Today `create_dropped_contract` produces the FreeAgent record at drop time with no "biddable from"
gating. The timing rule belongs to the auction/scheduler layer:

- The dropped FreeAgent contract's `effective_date` (from the drop's `deadline_model`, stamped on
  the team_update at `drop_contract_team_update.rs:53`) marks the drop week. The auction engine
  (spec 01) must treat the player as biddable only starting the **following** FA auction window.
- The scheduler (spec 05, `deadline_processing` / FA auction start deadlines) defines week
  boundaries; spec 01's "open auction" eligibility check must exclude players whose drop
  `effective_date` falls in the current FA week. Add the data needed (drop effective_date /
  drop-week index) to the dropped-contract lookup so the engine can apply the +1-week gate.
- Open question for spec 01/05: does "the week after" key off the calendar week of the drop or the
  next `Week*FreeAgentAuctionStart`/`InSeasonRosterLock` deadline? Resolve in spec 05's week model.

### Verify §9.1.1 scoping (after-keeper-deadline vs not-kept)

§9.1.1: the 20% penalty applies to a non-RD/RDI player dropped **after the keeper deadline**, and
explicitly **does not** include players merely not kept season-to-season.

Current scoping is correct and worth keeping documented:

- `calculate_team_contract_salary` short-circuits with **no penalty** when
  `deadline_model.kind == DeadlineKind::PreseasonKeeper` (`salary_calculation.rs:82-84`).
- `find_contracts_dropped_by_team_in_regular_season` (`contract_queries.rs:278`) filters out
  `DeadlineKind::{PreseasonStart, PreseasonKeeper}` (line ~299) — so drops at/before the keeper
  deadline are excluded from the penalty fold.
- `create_dropped_contract`'s `is_before_pre_season_keeper_deadline` branch resets dropped salary
  to $1 / Expired (no retained min bid, no penalty), matching "not kept" semantics.

**To verify:** that `is_preseason_keeper_or_before()`
(`drop_contract_from_team.rs:42`) and the deadline-kind filter together cover the full
keeper-deadline boundary identically, and that a drop *exactly at* the keeper deadline is treated
as "not kept" (no penalty), not as an in-season penalized drop. Add a regression test pinning a
drop at PreseasonKeeper → no penalty, and a drop at the first in-season deadline → penalty.

### GraphQL (cross-ref spec 06)

- Mutation `dropContract(contractId)` for the standard owner drop (penalty applied).
- Mutation `dropDeceasedPlayer(contractId)` (or `dropContract` with a `penaltyWaived` arg)
  guarded by commissioner role; returns the new cap with no penalty.
- Query field exposing a contract's projected drop penalty (`ceil(salary*0.2)`) and post-drop
  salary/cap so the frontend preview calls backend math, not its own (mirror spec 04's
  "one source of truth" rule).

## Frontend (Next.js + MUI v7)

### Drop confirmation with penalty preview; deceased-player penalty-waiver flow (commissioner)

- Drop action on a roster contract opens a confirmation dialog showing: player, current salary,
  computed penalty `ceil(salary*0.2)`, and resulting team salary/cap (values from the backend
  preview field, not recomputed client-side).
- For commissioners: a separate "drop deceased player (no penalty)" path (e.g. a checkbox or
  distinct menu item, visible only when `LeagueRole` is commissioner) that calls the waiver
  mutation and shows the penalty as $0. Surface a confirmation that this certifies the player as
  deceased and is audit-logged.
- Reflect the retained min-bid: when viewing a recently dropped player in FA, show the
  "minimum bid $N (previous salary)" badge sourced from the Active FreeAgent contract salary.

## Edge cases & open questions

- **Who marks a player deceased + audit:** only the commissioner; record authorizing user_id +
  timestamp on the drop transaction. PlayerStatus `Deceased` is a real-world flag (shared across
  leagues) but the *waiver* is a per-league transaction property — keep them separate.
- **Between-keeper-and-season-start window (§9.1.4):** a player who dies between keeper declaration
  and season start may be dropped penalty-free, but in that window
  `create_dropped_contract`'s pre-keeper-deadline branch already resets to $1/Expired with no
  penalty — so the waiver may be a no-op there. Verify the waiver flag still routes correctly
  (and that "between keeper deadline and season start" maps to deadlines after PreseasonKeeper but
  before the first in-season lock, which *would* otherwise penalize). Pin exact deadline kinds.
- **Min-bid persistence across drop→FA transition:** the retained salary lives on the FreeAgent
  contract record; confirm trades/re-signs of the FreeAgent in the same season don't silently
  drop the min (auction win creates a fresh V/1 per §9.2.1 / §11.8.2). Confirm annual advancement
  expires the FreeAgent record so the min clears at season end.
- **RD/RDI min-bid discrepancy:** §11.8.1 says dropped RD/RDI retain salary as min bid, but
  `create_dropped_contract` sets them to $1 — resolve before spec 01.

## Dependencies

- [spec 01](01-live-auction-engine.md) — auction min-bid: reads retained salary from Active
  FreeAgent contract; applies +1-week biddability gate.
- [spec 05](05-deadline-scheduler-and-transaction-processor.md) — scheduler/week model: defines
  the "week after" boundary for released-week-after timing and the keeper-deadline boundary.
- [spec 06](#) — GraphQL: drop mutations + commissioner authz + drop-penalty preview field.
