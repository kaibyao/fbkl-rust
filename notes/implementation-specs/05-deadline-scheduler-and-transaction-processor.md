# Spec 05 — Deadline Scheduler & Transaction Processor

**Rules ref:** §4.2, §8.1, §14.4 + all time-triggered events · **Status:** 🔴 stub crates · **Priority:** P0 (infra)

## Summary

The `logic/` crate already implements the *handlers* for most time-triggered league events
(`advance_league_contracts` on PreseasonStart, `lock_rosters`, `process_keeper_deadline_transaction`,
`end_veteran_auction`, `end_fa_auction`, draft-pick generation). What is missing is the **orchestration
layer** that fires these handlers when their `deadline` (`entity::deadline::Model`, keyed by
`DeadlineKind`) is reached. Today those handlers are only invoked by `import-data` replaying historical
CSVs — nothing runs them on a live clock.

This spec defines two crates that are currently empty stubs:

- **`transaction-processor/`** (`src/lib.rs` = `add(left, right)`): the *dispatcher* — given a due
  deadline or sub-event, run the correct `logic` fn inside one DB transaction, idempotently, and
  record the outcome.
- **`jobs/`** (`src/lib.rs` = `pub async fn process_keepers() {}`): the *scheduler* — discover due
  deadlines across all leagues, hand them to the processor, retry, and never double-fire.

⚠️ Root `CLAUDE.md` wrongly describes both crates as functional. See `notes/IMPLEMENTED.md` rows for
`transaction-processor` (14 LOC) and `jobs` (1 LOC).

This is P0 infra: specs **01** (live FA auction engine) and **03** (RFA resolution) cannot run on a
clock until this exists.

## Backend

### `transaction-processor/` crate

**Purpose:** given a `(deadline | sub-event)`, dispatch to the right `logic` fn, in a single
`db.begin()…commit()` DB transaction, idempotently, and record an outcome row.

Public surface (proposed):

```rust
pub async fn process_deadline<C: ConnectionTrait>(db: &C, deadline: &deadline::Model) -> Result<ProcessOutcome>;
pub async fn process_event<C>(db: &C, event: ProcessableEvent) -> Result<ProcessOutcome>;
```

- `ProcessableEvent` covers things that are *not* a row in the `deadline` table but are still
  time-triggered: an individual FA-auction 24h-no-bid close, an FA all-bid 30-min extension expiry
  (§8.3.2, spec 01), and an RFA 48h raise / 48h match window expiry (§15.3.2, spec 03). These derive
  their fire-time from `auction` / RFA-state rows, not from `deadline`.
- **One DB transaction per deadline/event.** Follow `logic/CLAUDE.md` convention #2: `db.begin()` →
  run handler (which itself inserts the `transaction` audit row + `team_update`s per convention #1) →
  insert/upsert a `job_run` outcome row → `commit()`. A handler failure must roll the whole thing back
  so re-fire is clean.
- **Idempotency:** before dispatching, check whether this deadline/event already has a `Succeeded`
  `job_run`. If so, no-op. The idempotency key is `(league_id, end_of_season_year, deadline_kind)` for
  deadline rows, or `(league_id, auction_id, sub_event_kind)` for sub-events (see entity section).
- **Outcome recording:** every run writes a `job_run` row (Pending → Running → Succeeded | Failed),
  capturing the dispatched handler, the resulting `transaction.id` (if any), and an error string on
  failure. This is the audit/recovery surface the commissioner console reads.
- The existing `transaction-processor` README and `add()` stub get deleted.

### `jobs/` crate (the scheduler)

**Purpose:** discover due deadlines across leagues and hand them to `transaction-processor`.

- **Mechanism — DB-driven poll, not cron.** Run a `tokio::time::interval` (e.g. every 30–60s) inside
  the long-lived `fbkl-server` process (`server/src/main.rs` already lists a "transaction-processor job"
  TODO at ~lines 87–112). Each tick:
  1. Query `deadline` for rows with `date_time <= now()` that lack a `Succeeded` `job_run`
     (a new `deadline_queries::find_due_unprocessed_deadlines(now)` joining against `job_run`).
  2. Query open `auction`s whose last bid is >24h old, or whose all-bid/extension window has expired,
     to synthesize `ProcessableEvent`s (cross-ref spec 01 for the timer model).
  3. Query pending RFA windows whose 48h timer has elapsed (cross-ref spec 03).
  4. For each, call `transaction_processor::process_deadline / process_event`.
- **Discovery is across all leagues** (multi-league, see edge cases) — `deadline.league_id` scopes each
  row; the poller is league-agnostic and processes every due row it finds.
- **Replace the empty `process_keepers()`** with `run_scheduler_tick(db)` (callable once for tests /
  manual trigger) and `spawn_scheduler(db)` (the interval loop). Keeper processing becomes one branch
  of the dispatch table, not a top-level fn.
- **Retries:** a `Failed` `job_run` is retried on the next tick up to `MAX_ATTEMPTS` (track `attempts`
  on `job_run`); after that it stays `Failed` and is surfaced to the commissioner console rather than
  silently retried forever. Transient DB errors retry; validation `bail!`s (e.g. an illegal roster at
  lock) are terminal and need a manual fix.
- **Idempotency at the scheduler layer** is the `job_run` existence check above — the interval may
  overlap a slow run, so processing must claim its `job_run` (status → `Running`) atomically before
  doing work, so a second tick skips it.

### Cap-by-period (centralized current-cap resolver)

§4.2 is a step function keyed off the most-recently-passed deadline:

- **$100** at/through `PreseasonKeeper` (keeper-eligible total only; not a real roster cap).
- **$200** (`PRE_SEASON_TOTAL_SALARY_LIMIT`) after keeper deadline, through veteran auction + rookie
  draft (`PreseasonVeteranAuctionStart` … `PreseasonFinalRosterLock`, `PreseasonFaAuctionStart/End`).
- **$210** (`REGULAR_SEASON_TOTAL_SALARY_LIMIT`) after auction+draft conclude — enables RD activations
  (§4.2.2, §11.4.3). Applies at `Week1*` and `InSeasonRosterLock` *before* FA freeze.
- **$230** (`POST_SEASON_TOTAL_SALARY_LIMIT`) after FA freeze (`FreeAgentAuctionEnd`, the $20 bump of
  §8.1/§4.2.3), through `TradeDeadlineAndPlayoffStart` and the playoffs.
- **no cap** between playoff conclusion (`SeasonEnd`) and the next keeper deadline (§4.2.4).

**This resolver already partially exists:** `entity::deadline::Model::get_salary_cap()`
(`entity/src/entities/deadline.rs:33-65`) selects the cap from `self.kind`, and for
`InSeasonRosterLock` it compares `self.date_time` against the `FreeAgentAuctionEnd` deadline to pick
$210 vs $230. **Do not introduce a second cap-selection path.** Instead:

1. Make this the single source of truth. The processor and any roster-lock handler must derive the cap
   from the deadline being processed via `get_salary_cap`, never from a local literal.
2. Fix the gap: `get_salary_cap` has no "no cap" state for the post-`SeasonEnd`→keeper window (§4.2.4) —
   it returns `POST_SEASON_TOTAL_SALARY_LIMIT` for `SeasonEnd` and falls through to
   `REGULAR_SEASON_TOTAL_SALARY_LIMIT` in the `_ =>` arm. Add an explicit "uncapped" representation
   (e.g. `Option<i16>` / `i16::MAX`) for the offseason window so roster legality isn't wrongly enforced.
3. The `$210→$230` transition is *event-driven* (the `FreeAgentAuctionEnd` deadline firing), so the
   processor's handler for `FreeAgentAuctionEnd` is what conceptually "applies" the bump — but because
   cap is resolved per-deadline at read time, no stored cap field needs mutating. Document that the
   bump is implicit in `get_salary_cap`'s `date_time > FreeAgentAuctionEnd` comparison.

### `entity/` (job-run / processing-status tracking)

New table **`job_run`** (+ migration in `migration/`, +`entity/src/queries/job_run_queries.rs`):

| col | type | note |
|-----|------|------|
| `id` | pk | |
| `league_id` | fk → league | |
| `end_of_season_year` | i16 | scopes to a season |
| `deadline_id` | fk → deadline, nullable | null for sub-events |
| `event_kind` | enum `JobEventKind` | `Deadline` \| `FaAuctionClose` \| `FaExtensionExpiry` \| `RfaRaiseWindow` \| `RfaMatchWindow` |
| `dispatch_target` | enum / string | which handler ran (mirrors `DeadlineKind` + sub-events) |
| `status` | enum `JobRunStatus` | `Pending` \| `Running` \| `Succeeded` \| `Failed` |
| `attempts` | i16 | retry counter |
| `idempotency_key` | string, **unique** | `(league_id, eos_year, kind[, auction_id])` rendered to a stable string; the unique index *is* the double-fire guard |
| `transaction_id` | fk → transaction, nullable | links the audit row the handler produced |
| `error` | text, nullable | failure detail for console |
| `created_at` / `updated_at` | tz | |

- **Idempotency** is enforced by the unique `idempotency_key` index — an attempted duplicate insert
  conflicts, so a concurrent/re-fired tick cannot double-process.
- **Partial-failure recovery — fix the swallow pattern.** `lock_rosters`
  (`logic/src/deadline_processing/roster_lock/`) currently **logs and swallows** future-draft-pick
  generation failure (`generate_future_draft_picks`), per `logic/CLAUDE.md` Gotchas and IMPLEMENTED.md
  §roster_lock. Under a real scheduler this hides a genuine failure. Change it to **propagate** the
  error so the wrapping DB transaction rolls back and the `job_run` records `Failed` — then it retries
  / surfaces to the console. Do the same for any other handler that currently downgrades a hard failure
  to a log line. (This is the canonical anti-pattern to eliminate, not copy.)
- Also relevant: `insert_team_updates_from_completed_trade` **panics** on missing pre-trade salary
  (`logic/CLAUDE.md` Gotchas). A panic in a scheduler worker is worse than a `bail!` — convert to a
  recoverable `Result` error so it lands as a `Failed` job_run, not a crashed tick.

### Event → logic-fn dispatch table

| Trigger | Handler (`logic/…`) | Notes |
|---------|---------------------|-------|
| `PreseasonStart` | `annual_contract_advancement::advance_league_contracts` | expire FAs, advance all other contracts a year (§14.2) |
| `PreseasonKeeper` | `deadline_processing::keeper_deadline::process_keeper_deadline_transaction` | §14.4; also announces RFAs/UFAs. Replaces stub `jobs::process_keepers` |
| `PreseasonVeteranAuctionStart` | (auction engine open, spec 01) | begin veteran auction; RFA week first (§6.3.1, spec 03) |
| `PreseasonFaAuctionStart` | (open FA bidding, spec 01) | open nominations begin after last predetermined player (§6.3.2) |
| `PreseasonFaAuctionEnd` | (close preseason FA nominations) | §6 preseason close |
| `PreseasonRookieDraftStart` | rookie-draft start; unlocks +2yr pick trading (§12.4) | rookie draft engine (out of scope here / spec TBD) |
| `PreseasonFinalRosterLock` | `deadline_processing::roster_lock::lock_rosters` | also runs `draft_picks::generate_future_draft_picks` (see swallow fix above) |
| `Week1FreeAgentAuctionStart` / `…End` | (open/close Week-1 FA auctions, spec 01) | Week-1 open time set in preseason (§8.1.2) |
| `Week1RosterLock` | `deadline_processing::roster_lock::lock_rosters` | first weekly lock at NBA tipoff (§3.2) |
| `InSeasonRosterLock` | `deadline_processing::roster_lock::lock_rosters` | weekly Monday lock; cap from `get_salary_cap` ($210 or $230) |
| `FreeAgentAuctionEnd` | FA-freeze handler — stop new FA nominations + apply $20 cap bump | §8.1.3 / §4.2.3. Cap bump is implicit in `get_salary_cap` (see Cap section) |
| `TradeDeadlineAndPlayoffStart` | trade-freeze handler | §12.3 no trades after; cap stays $230 |
| `SeasonEnd` | season-end handler — remove cap (§4.2.4), flip to offseason roster limits | enables `get_salary_cap` uncapped window |
| *sub-event* `FaAuctionClose` | `auction::end_fa_auction` | fired when open FA auction is 24h no-bid (§8.3.1, spec 01) |
| *sub-event* `FaExtensionExpiry` | `auction::end_fa_auction` | fired after the §8.3.2 30-min extension chain ends (spec 01) |
| *sub-event* veteran auction close | `auction::end_veteran_auction` | 24h no-bid close (§6.4.4, spec 01) |
| *sub-event* `RfaRaiseWindow` / `RfaMatchWindow` | RFA resolution (spec 03) | 48h winner-raise then 48h owner-match (§15.3.2) |

## Frontend (commissioner ops console)

A new commissioner-only section (gated on `LeagueRole`/admin). The team/league GraphQL surface needed
to drive this is itself partly unbuilt (`server/` team/contract resolvers are commented out — see
IMPLEMENTED.md §server), so this depends on that surface coming online.

- **Deadline calendar:** list this league's `deadline` rows (date_time, kind, name) with a
  passed/upcoming/processed badge (badge derived from matching `job_run.status`). All times shown in CT
  (see edge cases).
- **Manual "process now" trigger:** a mutation that calls `transaction_processor::process_deadline` for
  a selected deadline out of band (for backfill / when the poller is down). Must go through the same
  idempotency check so a manual run can't double-process an already-`Succeeded` deadline.
- **Job-run status / audit:** table of `job_run` rows (kind, status, attempts, linked `transaction_id`,
  timestamps) — the live view of what the scheduler has done.
- **Error surfacing:** `Failed` job_runs shown prominently with their `error` text and a manual
  "retry" action; this is how a commissioner unblocks an illegal-roster lock or missing-salary failure.

## Edge cases & open questions

- **Timezone:** rules are written in **CT** (§8.2 opening bids Fri 11:59 PM CT, all bids Sun 8:00 PM CT).
  `deadline.date_time` is `DateTimeWithTimeZone`; all comparisons must be tz-aware and the poller must
  use absolute instants. Decide a canonical storage tz (UTC) and render CT in the console; account for
  DST when generating weekly `InSeasonRosterLock` / FA deadlines.
- **Multi-league:** the poller is league-agnostic (scoped per `deadline.league_id`); confirm one shared
  scheduler in `fbkl-server` is acceptable vs per-league. `job_run.league_id` keeps audits separate.
- **Replay vs live coexistence with `import-data`:** historical replay already calls the same `logic`
  handlers directly. Replay must **not** create `job_run` rows / must not be picked up by the live
  poller (e.g. only seed `deadline` rows for the *current* live season into the poll window, or mark
  replayed deadlines `Succeeded` up front). Open question: how to fence the boundary between
  replayed-history and live-present so the scheduler only fires future deadlines.
- **Idempotency on re-fire:** unique `idempotency_key` + claim-to-`Running` covers it, but define
  behavior when a handler partially committed before a crash (shouldn't happen given single-txn design,
  but document the recovery: a `Running` row older than a timeout is reclaimable).
- **Manual override:** commissioner "process now" and "retry" must share the idempotency path and write
  `job_run` rows (audit who/when), so manual and automatic processing are indistinguishable downstream.
- **Open:** RD-activation / RDI eligibility guards are missing (`logic/CLAUDE.md` #4); if the processor
  ever drives those automatically, the missing guards become a correctness hole.

## Dependencies

- **Unblocks** [spec 01](01-live-auction-engine.md) (auction close/extension timers need a scheduler)
  and [spec 03](03-rfa-resolution-and-compensation.md) (48h RFA windows).
- **Relates to** spec 06 (commissioner ops / GraphQL surface this console rides on) and spec 08.
- **Soft-depends on** the `server/` team/contract/trade GraphQL resolvers (currently commented out) for
  the frontend console.
