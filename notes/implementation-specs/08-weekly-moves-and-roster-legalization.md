# Spec 08 — Weekly Moves & Roster Legalization
**Rules ref:** §5, §10, §11.4, §13 · **Status:** 🟡 backend built; frontend tray and wizard not built · **Priority:** P1

## Summary

Every in-season state change (`drop_contract`, `move_contract_to_ir`, `activate_rookie`, trade
processing, auction wins) records a `team_update` row, and every row one submission writes shares
one `transaction_number`. That group is a **transaction**, the unit two rules judge (§13.1.6): T1,
the roster must be legal after each transaction; and T2, a contract acquired in a transaction may
not be dropped or moved to the IR in that same transaction. `validate_transaction`
(`logic/src/roster/transaction.rs`) runs both the moment a transaction is submitted, so a roster
may be *transiently illegal* part-way through one (§13.1.3) and never after one. The Monday lock is
the last check of the week, not the only one: `validate_league_rosters`
(`logic/src/deadline_processing/roster_lock/validate_rosters.rs`) runs T1 again there. T1 reads its
limits from the period the moves are made in (`find_governing_deadline`), so **season-start
legalization** (32→22, direct-to-IR allowed, §5.1.3 / §11.4.3) and **in-season** moves
(must-hit-22-man-first, §10.3.1) are judged by different limits.

The backend of this spec shipped: (1) the weekly-move grouping model where illegality is permitted
only *within* a transaction, (2) the season-start legalization limits with their direct-to-IR
allowance and $10 cap bump, and (3) in-season IR-accommodation sequencing. Still to build:
(4) RD/RDI overflow resolution at season start, and the frontend tray and wizard below.

## Backend

### Weekly-move model (entity: a "week" / pending-move grouping; reorderable; transient-illegality allowed; commit/legalize at Monday lock)

The natural anchor already exists: each weekly `deadline` of kind `Week1RosterLock` /
`InSeasonRosterLock` defines the Monday lock that closes a week. Group the week's moves under the
**upcoming** lock deadline rather than inventing free-floating "weeks".

- **New entity `roster_move` (or extend `team_update`)**: the cleanest path is to lean on the
  existing `team_update` rows, which already carry `effective_date` (stamped from a deadline, see
  `logic/CLAUDE.md` §7), `status` (`TeamUpdateStatus`), and `ContractUpdateType`
  (`Drop`/`ToIR`/`FromIR`/`ActivateRookie`/`AddViaAuction`/`ToRdi`/`FromRdi`/…). A week = the set
  of `team_update` rows for one team filed under the same lock deadline, whatever their status: a
  move is applied when it is submitted, and its status says whether the lock has settled it yet.
  `team_update_queries::find_team_updates_by_team(team_id, status, deadline_id)` reads them.
- **Reorderability (§13.1.1)**: introduce a `transaction_number: i16` (nullable, owner-assigned)
  on `team_update` so the UI can present and reorder the week's transactions. Rows sharing a value
  are one transaction, judged together; transactions apply in ascending order. Order is not
  cosmetic: it decides which transaction each accommodating drop lands in (see
  *Ordering independence* under Edge cases).
- **Transient illegality (§13.1.3)**: illegality is allowed *while a transaction is being
  applied*, never after one. Each mutator records its `team_update`, and the roster may be
  over 22 or over cap part-way through a transaction, but every transaction is validated the
  moment it is submitted. The Monday lock is the last check of the week, not the only one.
- **Commit / legalize at lock**: `lock_rosters`
  (`logic/src/deadline_processing/roster_lock/lock_rosters.rs`) signs any auction win the owner
  never picked up, then runs `validate_league_rosters` over each team's live contract rows. There is
  no projection to build, because a move is applied to those rows when it is submitted. A legal
  team's `team_update` rows for that deadline flip to `TeamUpdateStatus::Done`; an illegal team's
  stay `Pending` and its broken rules are recorded as `roster_lock_violation` rows (see Edge cases).

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
  from the cap tally *before* checking the activated RD/RDI player fits. Because the whole
  declaration is one transaction, validated once it is fully applied, the ordering independence
  falls out for free — no special simultaneity code is needed.

### In-season IR accommodation sequencing (§10.3 must-hit-22-man-first; drop-from-IR keeps penalty)

For `Week1RosterLock` / `InSeasonRosterLock`:

- **Must hit 22-man first (§10.3.1, §10.1.2)**: this is T2 applied to the IR. A contract acquired
  in a transaction via auction (`AddViaAuction`), trade (`AddViaTrade`) or rookie draft
  (`AddViaRookieDraft`) may not be moved to the IR in that same transaction; it may be moved to the
  IR in any later transaction, which is where §10.3.1's "accommodate on the 22-man first" lands.
  `validate_transaction` enforces it over the transaction's own `ContractUpdate` list, so no scan
  of earlier committed roster states is needed and none is done — an earlier design that scanned
  for a `Done` non-IR row passed or failed by accident depending on what other moves that week had
  already written.
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
- **No projection**: a move changes the contract rows as it is submitted, so `validate_team_roster`
  and the league-wide sweep read the team's live contracts. Nothing rebuilds an end-of-week roster
  from a move list.
- **Sequencing validators (new)**: `validate_ir_accommodation_in_week` and
  `validate_rd_overflow_resolved` operate on the week's move list + projected roster.
- Reuse constants only (`logic/CLAUDE.md`): `REGULAR_SEASON_VET_OR_ROOKIE_CONTRACTS_PER_ROSTER_LIMIT`
  (22), `REGULAR_SEASON_IR_CONTRACTS_PER_ROSTER_LIMIT` (1),
  `REGULAR_SEASON_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT` (6),
  `REGULAR_SEASON_INTL_ROOKIE_DEVELOPMENT_CONTRACTS_PER_ROSTER_LIMIT` (1),
  `REGULAR_SEASON_TOTAL_SALARY_LIMIT` (210), `POST_SEASON_TOTAL_SALARY_LIMIT` (230),
  `PRE_SEASON_CONTRACTS_PER_ROSTER_LIMIT` (32).
- Each mutator continues to record a `league_event` (`TeamUpdateToIr`, `TeamUpdateFromIr`,
  `TeamUpdateDropContract`, `RookieContractActivation`, `AuctionDone`, `Trade`) + `team_update`
  per `logic/CLAUDE.md` convention 1. The weekly model changes *when legality is checked*, not the
  audit-log shape.

### GraphQL (cross-ref spec 06)

Expose the weekly tray via the schema (see [spec 06](06-graphql-api-surface.md)):
- `query teamWeek(teamId, deadlineId)` → the team's active `contracts`, the week's moves grouped
  into `transactions` in the owner's order whatever their status, one `ruleLegality` flag per roster
  rule, and an overall `isLegal`. No projected roster: the moves are already applied, so `contracts`
  is the roster as it stands.
- `mutation reorderTransactions(teamId, deadlineId, orderedTransactions: [[Int]])` — each inner list
  is one transaction and its position becomes the `transaction_number` its moves store. It takes the
  upcoming lock's week only, the list must hold that week's whole move set, and every proposed
  transaction is re-judged against T2 with that lock's deadline kind. T1 is not re-run, because
  reordering cannot change the roster the week ends with and §13.1.1 lets an owner reorder freely
  (fbkl-rust-140.33 holds the open question).
- `mutation submitTransaction(teamId, deadlineId, moves: [RosterMove])` — one transaction of drops,
  IR moves and activations (`RosterMoveKind` is `DROP`, `MOVE_TO_IR`, `ACTIVATE_FROM_IR`,
  `ACTIVATE_ROOKIE`). Every row it writes shares one transaction number, and T1 and T2 judge them
  together before the database transaction commits. The season-start wizard batches its
  IR/activate/drop moves through this same mutation.
- Validation failures answer with a structured error: `ROSTER_ILLEGAL` carries a per-rule violation
  list naming the team and the contract, and the commissioner query
  `rosterLockViolations(deadlineId)` lists what each lock recorded.

### Atomic transaction submission (trades and FA pickups carry their accommodating drops)

T1 is checked after each transaction, so a transaction has to reach the validator whole. Every drop
made to accommodate a transaction belongs to that transaction (§13.1.5), so the owner submits those
drops together with the moves that need them, inside one database transaction:

- **Trades**: `proposeTrade` carries the proposer's accommodating drops and `acceptTrade` the
  accepter's. On accept, the trade legs and every side's drops apply together, then each involved
  team's legs plus that team's drops are validated as that team's transaction. A rejected accept
  persists nothing, which is what §12.5.3 (no multi-part trades executed at different times) asks
  for.
- **Free agent pickups**: `pickUpAuctionWins(deadlineId, dropContractIds, irContractIds)` signs every
  one of the owner's won-but-unsigned auctions for the week and applies the listed drops and moves to
  the IR as one transaction. A move to the IR frees an active roster slot, so the owner may declare
  it as the move that makes room (§13.1.5.4). All of
  the week's wins go on together or none do (§8.3.5), which is why §8.3.7's case is refused by T2
  rather than by a roster count.
- **Single moves**: a lone drop, IR move or activation is a transaction of one move and runs through
  the same validator.

Every `team_update` one submission writes shares one `transaction_number` value.

## Frontend (Next.js + MUI v7)

- **Weekly transaction tray**: a panel listing this week's `Pending` moves for the team, drag-to-
  reorder (writes `transaction_number` via `reorderTransactions`), each move showing its delta. A live
  **end-of-week legality preview** banner (green = legal at Monday lock; amber = transiently
  illegal now but fixable; red = will fail lock) driven by `teamWeek.ruleLegality` and
  `teamWeek.isLegal`, which judge the roster as it stands. Make explicit that amber is allowed
  (§13.1.3) and only red blocks the lock.
- **Season-start legalization wizard** (shown only at `PreseasonFinalRosterLock`): steps owner from
  32 → 22+1IR+6RD+1RDI. Surfaces injured players eligible for **direct-to-IR**, over-limit RD/RDI
  players with **drop (penalty-free) vs activate** choices, the **$10 cap bump** in the running cap
  figure, and the simultaneous IR-vacate-then-activate affordance (§11.4.3).
- **IR move UI with context-sensitive rules**: same button behaves differently by deadline —
  at season start it offers direct-to-IR; in-season it greys out direct-to-IR for a player acquired
  in the *current transaction* (T2) and prompts "must be on the 22-man first" (§10.3.1). A player
  acquired earlier in the same week may go to the IR in a later transaction. Drop-from-IR shows the pending §9
  penalty (§10.3.3).

## Edge cases & open questions

- **"Legal by Monday" enforcement on violation (§13.1.2/§13.2)**: the rules say illegal weekly
  sequences are "ruled on by the commissioner" and "reverted". Open question: does the system
  **auto-revert** the offending move(s) at lock, **hard-block** the lock until the owner fixes it,
  or **flag for commissioner ruling**? Proposal: block the lock for that team (leave its
  `team_update`s `Pending`), notify owner + commissioner, and expose a commissioner override —
  matches §13.1.2's human-ruling intent without silently dropping players. Needs sign-off.
- **Ordering independence — CORRECTED**: legality is computed per transaction, in order, so
  transaction order decides which transaction each accommodating drop lands in and is no longer
  purely presentational. The model change is `team_update.sequence` becoming
  `team_update.transaction_number`. Owners may still re-order a week's transactions freely
  (§13.1.1); what they cannot do is move a drop into the transaction that acquired the contract.
  The *audit log* and any FA-report email (§8.3.8, §12.2.3) read the same numbers.
- **Interaction with auction pickups won mid-week (§8.3.5–.7) — SETTLED**: a transaction is
  "a set of one team's moves in a week that are applied and judged as a unit" (§13.1.4), and all of
  a week's free agent adds are one transaction. Two rules govern every transaction (§13.1.6):
  **T1**, the roster must be legal after each transaction; and **T2**, a contract acquired in a
  transaction may not be dropped, or moved to the IR, in that same transaction — it may be dropped
  or moved to the IR in any later transaction. This spec's earlier proposal, "a contract added this
  week may not be the one dropped to make room for another contract added that week", was right in
  spirit and wrong in scope: the unit is the transaction, not the week. Dropping a contract added
  earlier in the week is legal once the drop falls in a later transaction, and §8.3.7's
  Mitchell/Alvarado case stays illegal because a week's adds are one transaction, so the drop
  shares a transaction with its add. `validate_transaction`
  (`logic/src/roster/transaction.rs`) enforces both rules. Cross-ref
  [spec 01](01-live-auction-engine.md).
- **Atomic trades vs transient illegality (§12.1.3)**: trades must remain legal "on their own
  merits" even though the surrounding week may be transiently illegal — confirm trade processing
  isn't accidentally validated against the mid-week illegal roster.
- **IR-salary exclusion timing**: at `PreseasonFinalRosterLock`, must the IR'd player's salary be
  excluded *before* checking the activated overflow RD/RDI fits (§11.4.3 simultaneity)? Yes per
  rules; ensure the cap tally excludes `is_ir` salary.

## Dependencies

- [spec 01](01-live-auction-engine.md) — auction pickups won mid-week feed the weekly tray and the
  same-transaction add-then-remove rule (T2).
- [spec 05](05-deadline-scheduler-and-transaction-processor.md) — the lock deadlines that bound each
  week and trigger legalization.
- [spec 07](07-trade-legality.md) — trade legality at processing time vs end-of-week roster legality.
- [spec 06](06-graphql-api-surface.md) — `teamWeek` query, `reorderTransactions` and
  `submitTransaction`, structured errors.
