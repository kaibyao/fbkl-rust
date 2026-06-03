# Spec 08 — Weekly Moves & Roster Legalization
**Rules ref:** §5, §10, §11.4, §13 · **Status:** 🔴 weekly model not built; lock validation exists · **Priority:** P1

## Summary

Today every in-season state change (`drop_contract`, `move_contract_to_ir`, `activate_rookie`,
trade processing, auction wins) commits a `team_update` with status `Pending`/`Done`
*immediately and independently*, and `validate_league_rosters`
(`logic/src/deadline_processing/roster_lock/validate_rosters.rs`) only checks legality **at the
lock deadline**. There is no concept of a **week** as a transaction container, no notion that
moves within a week are reorderable, and no allowance for a roster being *transiently illegal*
mid-week (§13.1.3 — e.g. won an auction before dropping the player to fit it). The lock validator
also applies one flat ruleset and can't tell **season-start legalization** (32→22, direct-to-IR
allowed, §5.1.3 / §11.4.3) apart from **in-season** moves (must-hit-22-man-first, §10.3.1).

This spec adds (1) a weekly-move grouping model where mid-week illegality is permitted and only
the *end-of-week committed* state must be legal, (2) a season-start legalization flow with its
special direct-to-IR + simultaneous IR/activate/$10-bump rule, (3) in-season IR-accommodation
sequencing, and (4) RD/RDI overflow resolution at season start.

## Backend

### Weekly-move model (entity: a "week" / pending-move grouping; reorderable; transient-illegality allowed; commit/legalize at Monday lock)

The natural anchor already exists: each weekly `deadline` of kind `Week1RosterLock` /
`InSeasonRosterLock` defines the Monday lock that closes a week. Group the week's moves under the
**upcoming** lock deadline rather than inventing free-floating "weeks".

- **New entity `roster_move` (or extend `team_update`)**: the cleanest path is to lean on the
  existing `team_update` rows, which already carry `effective_date` (stamped from a deadline, see
  `logic/CLAUDE.md` §7), `status` (`TeamUpdateStatus`), and `ContractUpdateType`
  (`Drop`/`ToIR`/`FromIR`/`ActivateRookie`/`AddViaAuction`/`ToRdi`/`FromRdi`/…). A week = the set
  of `team_update` rows for one team whose `effective_date` maps to the same upcoming lock
  deadline and whose `status = Pending`. Add a query
  `team_update_queries::find_pending_team_updates_for_team_and_deadline(team_id, deadline_id)`.
- **Reorderability (§13.1.1)**: introduce a `sequence: i16` (nullable, owner-assigned) on
  `team_update` so the UI can present and reorder moves, but legality is computed on the *final
  set*, not on intermediate orderings — order is cosmetic for the audit log, not a constraint.
- **Transient illegality (§13.1.3)**: NO per-move legality check fires for in-season weekly moves.
  Each mutator records its `team_update` as `Pending` and returns; the roster may be illegal
  (over 22, over cap) until the week closes. The single hard gate is the Monday lock.
- **Commit / legalize at lock**: extend `lock_rosters`
  (`logic/src/deadline_processing/roster_lock/lock_rosters.rs`). It already (a) calls
  `validate_league_rosters`, then (b) flips that deadline's `team_update` rows to
  `TeamUpdateStatus::Done`. Keep this; the change is that validation must now run against the
  **end-of-week projected roster** = current `Done` contracts + all this-week `Pending` moves
  applied. If validation fails, the week does not commit (see Edge cases).

### Season-start legalization flow (32→22+1IR+6RD+1RDI; direct-to-IR allowed ONLY here; §11.4.3 simultaneous IR+activate+$10 bump)

This is the `PreseasonFinalRosterLock` deadline. Distinct rules from in-season:

- **32 → 22 active + 1 IR + 6 RD + 1 RDI** (§5.1.2). Owner reduces via IR move, trades, or drops
  (drops take the §9 penalty as usual — `drop_contract` already records penalty data).
- **Direct-to-IR allowed here ONLY** (§5.1.3, §10.1.2). `move_contract_to_ir` today does not check
  whether the contract was first accommodated on the 22-man; that no-check behavior is *correct
  for this deadline*. Mark it explicitly (pass the deadline kind, see validator section) so the
  in-season variant can forbid it.
- **§11.4.3 simultaneous IR + activate + $10 bump**: the 22/1IR/6RD/1RDI declaration is treated as
  a single atomic legalization, NOT a sequence — so an owner may IR an injured player *and*
  activate an over-limit RD/RDI player into the cap/roster space the IR move vacated, plus use the
  $10 cap bump. Concretely: the cap used during `PreseasonFinalRosterLock` validation must be
  `REGULAR_SEASON_TOTAL_SALARY_LIMIT` ($210, already the +$10 over the $200 preseason cap in
  `constants/src/league_rules/config_settings.rs`), and the IR'd contract's salary must be excluded
  from the cap tally *before* checking the activated RD/RDI player fits. Because validation runs on
  the final projected set (not move-by-move), the ordering independence falls out for free — no
  special simultaneity code is needed beyond computing legality on the union.

### In-season IR accommodation sequencing (§10.3 must-hit-22-man-first; drop-from-IR keeps penalty)

For `Week1RosterLock` / `InSeasonRosterLock`:

- **Must hit 22-man first (§10.3.1, §10.1.2)**: a contract acquired this week via auction
  (`AddViaAuction`) or trade (`AddViaTrade`) may NOT appear in the same week's projected roster as
  `is_ir = true` unless it was first legally a non-IR member of the 22-man at some committed point.
  Practically: forbid a `ToIR` move on a contract whose *only* prior `team_update` this week is the
  acquisition (i.e. it never had a `Done` non-IR state on this team). Add this check to
  `move_contract_to_ir` gated on in-season deadline kind, or to a new validator pass over the
  week's projected moves.
- **Drop-from-IR keeps penalty (§10.3.3)**: dropping directly from IR is allowed without
  re-accommodating on the active roster, but the §9 20% penalty still applies. `drop_contract`
  must NOT waive the penalty for `is_ir` contracts (only RD/RDI drops are penalty-free, §9.1.5 /
  §11.8.1). Verify `drop_contract` keys penalty on `ContractKind` (RD/RDI exempt), not on `is_ir`.
- **IR'd traded player must re-activate (§10.3.2, §11.7)**: if an `is_ir` contract is traded, the
  acquiring side cannot keep it on IR — same must-hit-22-man rule applies on receipt.

### RD/RDI overflow resolution at season start

§11.4.2: in the offseason an owner may hold **>6 RD / >1 RDI** (acquired via trade after season
end, §11.9.4). `validate_roster_contract_type_limits_not_exceeded` already enforces the 32-cap for
preseason deadlines and 6/1 for regular-season deadlines — so the limit *is* enforced at
`PreseasonFinalRosterLock`. The gap is the **resolution affordance**: by season start each overflow
RD/RDI must be either (a) **dropped penalty-free** (RD/RDI exempt from §9 penalty) or
(b) **activated** to a Rookie contract via `activate_rookie`
(`logic/src/rookie_development_activation/`), which takes cap+roster space and converts to `R/1`
(§11.5). Activation interacts with §11.4.3: the activated player consumes the vacated-IR cap and
the $10 bump. No new limit logic; provide a legalization-wizard surface (frontend) and ensure
`activate_rookie` / `drop_contract` are callable as part of the season-start week.

### logic/ functions + validators (extend validate_league_rosters; distinguish season-start vs in-season rules)

- **`validate_league_rosters` signature**: it already receives the `deadline_model` and branches on
  `DeadlineKind`. Add an explicit split:
  - `validate_season_start_legalization(...)` for `PreseasonFinalRosterLock` — cap = $210, IR salary
    excluded, direct-to-IR allowed, 6/1 RD/RDI enforced, overflow must be resolved.
  - `validate_in_season_week(...)` for `Week1RosterLock` / `InSeasonRosterLock` /
    `FreeAgentAuctionEnd` / `TradeDeadlineAndPlayoffStart` — enforces must-hit-22-man-first for
    newly-acquired contracts before IR, drop-from-IR penalty, cap per period
    ($210 → $230 after `FreeAgentAuctionEnd`, see `POST_SEASON_TOTAL_SALARY_LIMIT`).
- **Project end-of-week roster**: new helper
  `project_roster_at_lock(team_id, lock_deadline, db) -> Vec<contract::Model>` = committed `Done`
  contracts with this week's `Pending` `team_update` moves applied (drop removes, ToIR flips
  `is_ir`, activate changes `ContractKind`, adds insert). Feed this into the existing
  `validate_roster_*` helpers instead of the raw `find_active_contracts_in_league` set.
- **Sequencing validators (new)**: `validate_ir_accommodation_in_week` and
  `validate_rd_overflow_resolved` operate on the week's move list + projected roster.
- Reuse constants only (`logic/CLAUDE.md`): `REGULAR_SEASON_VET_OR_ROOKIE_CONTRACTS_PER_ROSTER_LIMIT`
  (22), `REGULAR_SEASON_IR_CONTRACTS_PER_ROSTER_LIMIT` (1),
  `REGULAR_SEASON_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT` (6),
  `REGULAR_SEASON_INTL_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT` (1),
  `REGULAR_SEASON_TOTAL_SALARY_LIMIT` (210), `POST_SEASON_TOTAL_SALARY_LIMIT` (230),
  `PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT` (32).
- Each mutator continues to record a `transaction` (`TeamUpdateToIr`, `TeamUpdateFromIr`,
  `TeamUpdateDropContract`, `RookieContractActivation`, `AuctionDone`, `Trade`) + `team_update`
  per `logic/CLAUDE.md` convention 1. The weekly model changes *when legality is checked*, not the
  audit-log shape.

### GraphQL (cross-ref spec 06)

Expose the weekly tray and projection via the schema (see [spec 06](06-graphql-api-surface.md)):
- `query teamWeek(teamId, deadlineId)` → committed roster + this-week `Pending` moves + projected
  end-of-week roster + per-rule legality flags (overCap, over22, ir/rd/rdi counts).
- `mutation reorderWeeklyMoves(teamId, orderedTeamUpdateIds)` — sets `sequence`.
- `mutation legalizeRoster(teamId, moves[])` for the season-start wizard (batch IR/activate/drop).
- Lock-time validation failures surface as structured errors (which rule, which contract).

## Frontend (Next.js + MUI v7)

- **Weekly transaction tray**: a panel listing this week's `Pending` moves for the team, drag-to-
  reorder (writes `sequence` via `reorderWeeklyMoves`), each move showing its delta. A live
  **end-of-week legality preview** banner (green = legal at Monday lock; amber = transiently
  illegal now but fixable; red = will fail lock) driven by `teamWeek.projected`. Make explicit that
  amber is allowed (§13.1.3) and only red blocks the lock.
- **Season-start legalization wizard** (shown only at `PreseasonFinalRosterLock`): steps owner from
  32 → 22+1IR+6RD+1RDI. Surfaces injured players eligible for **direct-to-IR**, over-limit RD/RDI
  players with **drop (penalty-free) vs activate** choices, the **$10 cap bump** in the running cap
  figure, and the simultaneous IR-vacate-then-activate affordance (§11.4.3).
- **IR move UI with context-sensitive rules**: same button behaves differently by deadline —
  at season start it offers direct-to-IR; in-season it greys out direct-to-IR for a player acquired
  this week and prompts "must be on the 22-man first" (§10.3.1). Drop-from-IR shows the pending §9
  penalty (§10.3.3).

## Edge cases & open questions

- **"Legal by Monday" enforcement on violation (§13.1.2/§13.2)**: the rules say illegal weekly
  sequences are "ruled on by the commissioner" and "reverted". Open question: does the system
  **auto-revert** the offending move(s) at lock, **hard-block** the lock until the owner fixes it,
  or **flag for commissioner ruling**? Proposal: block the lock for that team (leave its
  `team_update`s `Pending`), notify owner + commissioner, and expose a commissioner override —
  matches §13.1.2's human-ruling intent without silently dropping players. Needs sign-off.
- **Ordering independence**: legality computed on the projected union, so within-week order never
  changes the lock outcome. But the *audit log* and any FA-report email (§8.3.8, §12.2.3) should
  still reflect a coherent order — `sequence` is for presentation only; confirm that's acceptable.
- **Interaction with auction pickups won mid-week (§8.3.5–.7)**: a won auction contract is added
  `Pending` and **cannot be dropped to accommodate another pickup from the same week** (§8.3.7
  example). The week's move validator must enforce: a contract added this week may not be the one
  dropped to make room for another contract added this week. Cross-ref [spec 01](01-live-auction-engine.md).
- **Atomic trades vs transient illegality (§12.1.3)**: trades must remain legal "on their own
  merits" even though the surrounding week may be transiently illegal — confirm trade processing
  isn't accidentally validated against the mid-week illegal roster.
- **IR-salary exclusion timing**: at `PreseasonFinalRosterLock`, must the IR'd player's salary be
  excluded *before* checking the activated overflow RD/RDI fits (§11.4.3 simultaneity)? Yes per
  rules; ensure the projection excludes `is_ir` salary in the cap tally.

## Dependencies

- [spec 01](01-live-auction-engine.md) — auction pickups won mid-week feed the weekly tray and the
  "can't-drop-same-week-add" rule.
- [spec 05](05-deadline-scheduler-and-transaction-processor.md) — the lock deadlines that bound each
  week and trigger legalization.
- [spec 07](07-trade-legality.md) — trade legality at processing time vs end-of-week roster legality.
- [spec 06](06-graphql-api-surface.md) — `teamWeek` query, reorder/legalize mutations, structured
  lock errors.
