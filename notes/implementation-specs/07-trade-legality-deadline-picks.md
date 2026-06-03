# Spec 07 — Trade Legality, Deadline & Picks

**Rules ref:** §12 · **Status:** 🟡 core trades work; legality/deadline/conditions missing · **Priority:** P1

## Summary

Propose/accept/process is solid (`logic/src/trade/`): `propose_trade`, `accept_trade`
(auto-processes once every involved team's latest `trade_action` is `Propose`/`Accept`),
`process_trade`, multi-owner + one-way trades, and `external_trade_invalidation`. **Do not respec
that.** This spec adds the missing *gating* and *resolution* around it:

1. **Trade-time legality** — trades validate asset *ownership only* (`validate_trade_assets`); no
   cap or roster-size check (§12, §4.1).
2. **Trade deadline** (§12.3) — `DeadlineKind::TradeDeadlineAndPlayoffStart` exists but nothing
   blocks proposing/accepting after it; no trades until after the playoffs.
3. **Pick tradability window** (§12.4) — picks tradable up to 2 years out; "next year" begins the
   moment the Rookie Draft concludes. `generate_future_draft_picks` makes N+2 picks but the
   eligibility window isn't enforced when proposing.
4. **Conditional trades** (§12.5.1) — only draft-pick *position* conditions allowed. `draft_pick_option`
   table + `Proposed`/`Active`/`Used` statuses exist, but the *resolution* logic (evaluate the
   position condition once the pick slot is known, then transfer the correct pick) is unbuilt.
5. **Harden `insert_team_updates_from_completed_trade`** — currently `.expect()` panics on a missing
   pre-trade salary.

Out of scope (cross-ref [spec 12](12-out-of-scope-and-external.md)): §12.5.3 multi-part / players-to-be-named-later
(not allowed), §12.5.4 rentals (not allowed), §12.6 collusion (commissioner discretion, no auto-veto).

## Backend

### Trade-time legality validation (cap + roster)

Today `validate_trade_assets` (`logic/src/trade/validate_trade_assets.rs`) only checks:
contract latest-in-chain + owned by `from_team`; draft pick owned by `from_team`; option `Proposed`.
No post-trade cap or roster-size check on either side.

- **Decision: warn at propose-time, block only at process-time — and even then defer cap.** §13
  explicitly permits *transient* mid-week illegality (e.g. winning an auction before dropping to
  open a slot), and §12.1.3 requires a trade be legal "on its own merits" but the *roster* is
  reconciled by week-end roster lock. So:
  - **Roster-size legality is NOT a hard block on a trade.** It's resolved at the week-end lock
    (`validate_league_rosters` in `roster_lock/validate_rosters.rs`, already enforces RD≤6 / RDI≤1 /
    vet+rookie≤22 + cap). A trade that leaves a team over a roster limit is legal mid-week as long
    as it's reconciled (drop/IR/further trade) before the next `InSeasonRosterLock`.
  - **Cap legality**: same treatment — do not hard-block in `process_trade`. Compute and surface the
    *projected* post-trade salary/cap per side (we already compute both pre- and post-trade salaries
    in `process_trade` via `calculate_team_contract_salary` and `create_trade_team_update.rs`).
- **Implementation**: add a `validate_trade_legality` helper alongside `validate_trade_assets` that
  reuses `roster::calculate_team_contract_salary` for each `to_team` post-trade and the per-type
  count logic from `validate_roster_contract_type_limits_not_exceeded` /
  `validate_roster_ir_slot_limits` (refactor those out of `validate_rosters.rs` into shared fns so
  trade and roster-lock share one source of truth — do not duplicate the limit literals; they come
  from `constants::config_settings`). Return a structured `TradeLegalityReport { over_cap_by, over_roster_by }`
  per team rather than a `bail!`, so the GraphQL layer can render a non-blocking warning.
- **Do NOT block** on transient illegality. The only hard `bail!` additions in this spec are the
  deadline gate and the pick-window check below (both are absolute rule violations, not transient).

### Trade deadline gate (`DeadlineKind::TradeDeadlineAndPlayoffStart`)

§12.3: deadline = roster lock the first week of the playoffs; no trades until after the playoffs
(`SeasonEnd`). Nothing enforces this today.

- Add `validate_trade_window_open(league_id, end_of_season_year, action_datetime, db)` (new fn in
  `logic/src/trade/`). Reject if `action_datetime` falls in the closed window:
  `TradeDeadlineAndPlayoffStart ≤ action_datetime < SeasonEnd`. Resolve both deadlines via
  `deadline_queries::find_deadline_for_season_by_type` and compare against
  `find_most_recent_deadline_by_datetime` / `find_next_deadline_for_season_by_datetime`.
- Call it in **both** `propose_trade` (using a propose datetime — propose currently takes none; thread
  one through) **and** `accept_trade` (uses `accept_datetime`). Both must gate, because a trade
  proposed before the deadline must not be *accepted/processed* after it.
- `bail!` with a clear message ("trades are closed from the playoff trade deadline until season end").

### Pick tradability window (§12.4 two-year rule; window resets after Rookie Draft)

§12.4: picks tradable up to two years out; the "next year" begins immediately after the Rookie
Draft concludes. `draft_pick.end_of_season_year` is the discriminator; `FUTURE_DRAFT_PICK_SEASONS_LIMIT = 2`.

- Add `validate_draft_pick_trade_asset` window check (extend the existing fn in
  `validate_trade_assets.rs`, which today only checks ownership). A pick with `end_of_season_year`
  is tradable iff it is within the open window for the trade's datetime:
  - **Before that year's Rookie Draft concludes** (`PreseasonRookieDraftStart`/its end deadline for
    the current `end_of_season_year`): the current draft year + next year are tradable
    (`current_year` and `current_year + 1`).
  - **After the Rookie Draft concludes**: the window advances — `current_year + 1` and
    `current_year + 2` (the just-completed year's picks are spent / no longer tradable).
  - Concretely: `pick.end_of_season_year` must be `> latest_completed_draft_year` and
    `≤ latest_completed_draft_year + FUTURE_DRAFT_PICK_SEASONS_LIMIT`, where
    `latest_completed_draft_year` is derived from whether the Rookie Draft for the season containing
    the trade datetime has concluded (deadline lookup as above).
- `bail!` on out-of-window picks (this is an absolute rule violation, not transient).
- Apply the same check to `draft_pick_option` assets via their referenced pick
  (`draft_pick_queries::get_draft_picks_affected_by_options`).

### Conditional draft-pick trades (`draft_pick_option` resolution)

The option lifecycle today: created `Proposed` in a proposal → `process_trade` flips it to `Active`
(`process_trade_assets`) → external invalidation can set `InvalidatedByExternalTrade` /
`CancelledViaTradeRejection`. The terminal `Used` status is defined but **never set** — no code
evaluates the condition and transfers the conditioned pick. That's the gap.

- **Condition representation**: `draft_pick_option.clause` is a free-text `String` today. Replace
  (or back) it with a structured, position-only condition so it can be machine-evaluated, e.g. a
  `ConditionalPickClause { source_draft_pick_id, if_position_in: RangeInclusive<i16>, then_pick_ids,
  else_pick_ids }` serialized to the `clause` column (keep the human string for display). Per §12.5.1
  **only draft-pick position** conditions are allowed — no player/team performance. Reject anything
  else at propose-time.
- **Trigger / timing**: the condition can only resolve once the *source* pick's final position is
  known. The source pick's slot is determined by the standings/lottery feeding the Rookie Draft
  ([spec 02](02-rookie-draft-engine.md)). Add a resolution step that runs when draft order is finalized
  (the lottery/seeding step in spec 02), iterating `Active` options whose source pick now has a known
  position:
  1. Evaluate `if_position_in` against the resolved position.
  2. Transfer the correct pick set (`then_pick_ids` vs `else_pick_ids`) by reassigning
     `draft_pick.current_owner_team_id` to the option's beneficiary (mirror the reassignment in
     `process_trade_assets`).
  3. Set the option `Used`; record a `transaction` + per-team `team_update`
     (`DraftPickUpdateType::DraftPickOptionAdded` already exists — add a "resolved/used" variant) so
     the audit log + UI reflect it. Wrap in a `db.begin()`/`commit()` per `logic/CLAUDE.md` convention.
- Edge: the two examples in §12.5.1 both transfer *different pick bundles* depending on a range
  (1-3 vs else; 7-12 vs else) — model `then`/`else` as pick *sets*, not single picks.

### Harden `insert_team_updates_from_completed_trade` panic

`logic/src/trade/create_trade_team_update.rs:95-98` does
`team_salaries_before_trade.get(&team_id).expect(...)`. If a `to_team` that receives an asset never
had its salary precomputed (e.g. a team that only *receives* and whose id wasn't in the pre-trade
salary map), this panics.

- Replace `.expect()` with `.ok_or_else(|| eyre!(...))?` so a missing salary is a recoverable error,
  not a panic. (`process_trade` builds the map from `all_team_ids` = union of every `from_team_id`
  and `to_team_id`, so it *should* be complete — but defend it anyway; the panic is the documented
  gotcha in `logic/CLAUDE.md`.)
- Add a regression test: a one-way trade to a team that owns no contracts (empty `EMPTY_VEC` path)
  must still produce a valid `team_update`, not panic.

## Frontend (Next.js + MUI v7 + urql)

Note: per `IMPLEMENTED.md`, team/player/contract GraphQL resolvers are commented out — these depend
on [spec 06](#dependencies) wiring trade queries/mutations first.

- **Trade builder UI**: multi-team asset picker (contracts + picks + conditional options), reflecting
  one-way and multi-owner trades. Asset lists filtered to assets the `from_team` actually owns
  (latest-in-chain contracts, in-window picks only).
- **Legality preview**: render the `TradeLegalityReport` per team — projected post-trade salary/cap
  and roster counts, with a **non-blocking warning** (not an error) when a side ends over cap/roster,
  worded to reflect §13's "reconcile by week-end lock" rule.
- **Conditional-pick condition editor**: position-range builder only (e.g. "picks 1-3 → bundle A,
  else bundle B"). No player/team performance inputs (disallowed by §12.5.1).
- **Deadline-closed state**: when the trade window is closed (between `TradeDeadlineAndPlayoffStart`
  and `SeasonEnd`), disable propose/accept actions and show why.

## Edge cases & open questions

- **Transient-illegality window**: confirm with commissioner that trade-time cap/roster overage is a
  *warning*, reconciled by the next roster lock, vs a hard block. This spec assumes warn (§13). If the
  league wants hard blocks, flip the legality helper to `bail!`.
- **Condition evaluation timing**: resolution depends on the lottery/seeding step that finalizes the
  Rookie Draft order (spec 02). If an `Active` option's source pick is itself traded again before
  resolution, ensure the beneficiary tracks the *option*, not the team-at-proposal-time.
- **Picks acquired after a bid (spec 03 interplay)**: §15.2 forbids forfeiting RFA-compensation picks
  acquired *after* the winning bid. A pick mid-flight in a conditional option must not double as an
  eligible compensation pick — coordinate the "acquired-after" timestamp logic with
  [spec 03](03-rfa-resolution-and-compensation.md).
- **Window boundary precision**: the §12.4 window flips exactly at Rookie Draft *conclusion*. Confirm
  which deadline marks "conclusion" (`PreseasonRookieDraftStart` + draft duration vs a dedicated end
  deadline) so the window-advance is unambiguous.

## Dependencies

- [spec 02](02-rookie-draft-engine.md) — draft order / lottery finalization triggers conditional-pick resolution.
- [spec 03](03-rfa-resolution-and-compensation.md) — shared "pick acquired after a point in time" eligibility logic.
- [spec 05](05-deadline-scheduler-and-transaction-processor.md) — deadline scheduling powers the trade-deadline gate.
- [spec 06](#) — GraphQL trade queries/mutations (team/player/contract resolvers currently disabled).
- [spec 08](#) — weekly-moves legality (week-end roster lock that reconciles transient trade illegality).
