# Spec 05 — Deadline Scheduler & Transaction Processor

**Rules ref:** §4.2, §8.1, §14.4 + all time-triggered events · **Status:** 🟡 both crates built; commissioner console not built · **Priority:** P0 (infra)

## Summary

The `logic/` crate contains the *handlers* for the time-triggered league events
(`advance_league_contracts` on `PreseasonStart`, `lock_rosters`, `process_keeper_deadline_league_event`,
`assemble_veteran_auction_pool`, `end_veteran_auction`, `end_fa_auction`, the RFA window handlers).
This spec covers the two crates that run those handlers when their `deadline`
(`entity::deadline::Model`, keyed by `DeadlineKind`) or a synthesized sub-event comes due.

Both crates are built:

- **`transaction-processor/`**: the *dispatcher* — given a due deadline or sub-event, claim a
  `job_run`, run the matching `logic` fn inside one DB transaction, and record the outcome.
- **`jobs/`**: the *scheduler* — discover due work across all leagues, pass it to the processor, and
  count what the tick did.

The tick is a database poll in both runtimes; only the caller differs. Local development spawns the
30-second loop inside the long-lived server process (`fbkl_jobs::spawn_scheduler`, called from
`server/src/main.rs:46`). Production runs one tick per invocation from the `fbkl-scheduler` Lambda
(`lambdas/src/bin/scheduler.rs`), which `EventBridge` Scheduler calls every minute and which never
calls `spawn_scheduler`, because a Lambda cannot hold a loop open. A double invocation is safe: the
`job_run` claim is the double-fire guard.

Historical replay (`import-data`) calls the same `logic` handlers directly and writes no `job_run`
rows, so a replayed deadline still reads as unprocessed to this poller (see the open questions below).

## Backend

### `transaction-processor/` crate

**Purpose:** given a `(deadline | sub-event)`, dispatch to the right `logic` fn inside one DB
transaction, idempotently, and record an outcome row.

Public API:

```rust
pub async fn process_deadline<C>(db: &C, deadline_model: &deadline::Model) -> Result<ProcessOutcome>
where C: ConnectionTrait + TransactionTrait;
pub async fn process_event<C>(db: &C, event: ProcessableEvent) -> Result<ProcessOutcome>
where C: ConnectionTrait + TransactionTrait;
```

- `ProcessableEvent { league_id, end_of_season_year, subject_id, kind }` covers what is *not* a row in
  the `deadline` table but is still time-triggered. `ProcessableEventKind` has five variants:
  `FaAuctionClose`, `FaExtensionExpiry`, `VeteranAuctionClose`, `RfaRaiseWindowExpiry` and
  `RfaMatchWindowExpiry`. `subject_id` names an `auction` row for the three closes and an
  `rfa_resolution` row for the two RFA expiries. `close_at` already includes the §8.3.2 all-bid
  extension chain, so a rolled deadline is not a separate event kind.
- **Claim, dispatch, record.** `claim_job_run` inserts (or reclaims) the `job_run` row and commits it
  outside the handler's DB transaction, so a concurrent tick reads `Running` and skips. The handler
  then runs inside `db.begin()` … `commit()`. On success the `Succeeded` write shares that commit, so
  the handler's effects and its outcome are committed together; on failure the transaction rolls back
  and the `Failed` write goes through the outer connection.
- **Idempotency:** the claim decides. A `Succeeded` row gives `AlreadyProcessed`, a recent `Running`
  row gives `AlreadyRunning`, and a `Failed` row that has used `MAX_ATTEMPTS` (5) gives
  `AttemptsExhausted`. A `Running` row untouched for `STALE_RUNNING_TIMEOUT_MINUTES` (10) counts as an
  abandoned worker and is reclaimed under a conditional update, so only one reclaimer wins. The key is
  `league:eos:kind:deadline-<id>` for a deadline row and `league:eos:kind:auction-<id>` or
  `…:rfa-resolution-<id>` for a sub-event: it names the row and not only the kind, because every
  weekly lock shares `InSeasonRosterLock`.
- **Outcome recording:** `ProcessOutcome` is `Processed`, `AlreadyProcessed`, `AlreadyRunning`,
  `AttemptsExhausted` or `Failed { error }`. Every attempt leaves a `job_run` row holding the
  dispatched handler, the attempt count and the error string, which is what the commissioner console
  reads. The `league_event_id` column is there for the audit row a handler produced, but no dispatch
  fills it in yet.

### `jobs/` crate (the scheduler)

**Purpose:** discover due work across leagues and pass it to `transaction-processor`.

- **Mechanism — a database poll.** `run_scheduler_tick(db)` runs one pass and returns a `TickSummary`
  (`processed`, `failed`, `skipped`, `blocked`, `errors`). `spawn_scheduler(db)` wraps that call in a
  `tokio::time::interval` of `SCHEDULER_TICK_INTERVAL_SECS` (30) for the local server; the Lambda
  calls the one-shot tick. A tick that runs long resumes at the next scheduled time.
- **Order inside one tick** (`jobs/src/lib.rs`):
  1. `run_veteran_auction_release_tick` — open the schedule rows due today, move unbid auctions down a
     tier (§6.3.3-.5), and shorten the reprieve of auctions inside the crunch window (§6.4.4). The
     tier move runs before the close because it is an unbid auction's only clock. Every step here is
     idempotent by construction, so it takes no `job_run`.
  2. `run_auction_close_tick` — close every auction whose `close_at` has passed. It runs before the
     deadline loop because an in-season FA auction's close is limited to the upcoming roster lock, so
     the win has to be recorded before that lock reads the week's wins.
  3. The due-deadline loop — `deadline_queries::find_due_unprocessed_deadlines(now)` returns every
     league's due rows oldest-first; the tick buckets them per league and works each league's rows in
     that order. A league stops at the first deadline that does not reach `Succeeded` and counts the
     rest as `blocked`, because a later lock builds on an earlier one. Leagues are independent, so one
     stuck league never holds up another.
  4. `run_rfa_window_tick` — expire the RFA raise and match windows whose 48h timer has run out
     (§15.3.2, spec 03).
- **Discovery is across all leagues** — `deadline.league_id` scopes each row; the poller is
  league-agnostic and processes every due row it finds.
- **Retries:** the processor's claim reclaims a `Failed` run on a later tick until `MAX_ATTEMPTS` (5),
  after which it stays `Failed` and needs a manual retry from the commissioner console. Nothing sorts
  errors into transient and terminal today: an illegal roster at a lock gets the same five attempts a
  dropped connection does.
- **Idempotency at the scheduler layer** is the processor's claim. The scheduler keeps no state
  between ticks, so a manual trigger and an overlapping tick use the same code.

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

**This resolver is built:** `entity::deadline::Model::get_salary_cap()`
(`entity/src/entities/deadline.rs`) selects the cap from `self.kind`, and for
`InSeasonRosterLock` it compares `self.date_time` against the `FreeAgentAuctionEnd` deadline to pick
$210 vs $230. **Do not introduce a second cap-selection path.** Instead:

1. Make this the single source of truth. The processor and any roster-lock handler must derive the cap
   from the deadline being processed via `get_salary_cap`, never from a local literal.
2. Built: `get_salary_cap` returns `Option<i16>`, and `None` is the uncapped window. Only
   `PreseasonStart` resolves to `None` today, and `PreseasonFinalRosterLock` resolves to $210, so
   which kind maps to which cap still disagrees with the step function above. Tracked as
   fbkl-rust-140.39.
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
| `event_kind` | enum `JobEventKind` | `Deadline` \| `FaAuctionClose` \| `FaExtensionExpiry` \| `VeteranAuctionClose` \| `RfaRaiseWindow` \| `RfaMatchWindow` |
| `dispatch_target` | enum / string | which handler ran (mirrors `DeadlineKind` + sub-events) |
| `status` | enum `JobRunStatus` | `Pending` \| `Running` \| `Succeeded` \| `Failed` |
| `attempts` | i16 | retry counter |
| `idempotency_key` | string, **unique** | `league:eos:kind:<table>-<row id>`; the unique index *is* the double-fire guard |
| `league_event_id` | fk → league_event, nullable | links the audit row the handler produced |
| `error` | text, nullable | failure detail for console |
| `created_at` / `updated_at` | tz | |

- **Idempotency** is enforced by the unique `idempotency_key` index — an attempted duplicate insert
  conflicts, so a concurrent/re-fired tick cannot double-process.
- **Partial-failure recovery.** A handler must return its errors rather than log them, so the DB
  transaction rolls back and the `job_run` records `Failed` for the console to show. The
  `generate_future_draft_picks` call inside `lock_rosters` is commented out
  (`logic/src/deadline_processing/roster_lock/lock_rosters.rs`), so a final preseason lock generates
  no picks at all today; when it is restored it must return its error rather than log it.
- `insert_team_updates_from_completed_trade` now returns a typed `MissingPreTradeSalary` error rather
  than panicking, so a missing pre-trade salary records a `Failed` job run instead of stopping the
  worker.

### Event → logic-fn dispatch table

| Trigger | Handler (`logic/…`) | Notes |
|---------|---------------------|-------|
| `PreseasonStart` | `annual_contract_advancement::advance_league_contracts` | expire FAs, advance all other contracts a year (§14.2) |
| `PreseasonKeeper` | `deadline_processing::keeper_deadline::process_keeper_deadline_league_event` | §14.4; also announces RFAs/UFAs |
| `PreseasonVeteranAuctionStart` | `auction::assemble_veteran_auction_pool` | writes the season's release schedule; RFA week first (§6.3.1, spec 03). The tick then opens each row on its date |
| `PreseasonFaAuctionStart` / `…End` | recorded, no handler | preseason nominations (§6.3.2); the tick opens and closes the auctions |
| `PreseasonRookieDraftStart` | recorded, no handler | the draft starts from the commissioner mutation; scheduler wiring is fbkl-rust-z2c |
| `PreseasonFinalRosterLock` | `deadline_processing::roster_lock::lock_rosters` | the `draft_picks::generate_future_draft_picks` call is commented out, so no picks are generated |
| `Week1FreeAgentAuctionStart` / `…End` | recorded, no handler | Week-1 open time set in preseason (§8.1.2); the tick opens and closes the auctions, and `open_in_season_fa_auction` checks the §8.2 nomination window |
| `Week1RosterLock` | `deadline_processing::roster_lock::lock_rosters` | first weekly lock at NBA tipoff (§3.2) |
| `InSeasonRosterLock` | `deadline_processing::roster_lock::lock_rosters` | weekly Monday lock; cap from `get_salary_cap` ($210 or $230) |
| `FreeAgentAuctionEnd` | recorded, no handler | §8.1.3 / §4.2.3: the $20 cap bump is resolved at read time by `get_salary_cap`, and `end_fa_auction` refuses a close after the freeze |
| `TradeDeadlineAndPlayoffStart` | recorded, no handler | §12.3 no trades after; the freeze is checked when a trade is processed |
| `SeasonEnd` | recorded, no handler | §4.2.4: the cap change is resolved at read time by `get_salary_cap` |
| *sub-event* `FaAuctionClose` / `FaExtensionExpiry` | `auction::end_fa_auction` | runs against `find_upcoming_roster_lock`, so the win joins the week that lock checks; refused when the season has no lock left to fire. Only `FaAuctionClose` is synthesized today: `close_at` already includes the extension chain |
| *sub-event* `VeteranAuctionClose` | `auction::end_veteran_auction` | same `close_at` rule; an unbid auction expires at the bottom tier (spec 01) |
| *sub-event* `RfaRaiseWindow` | `deadline_processing::decline_to_raise` | §15.3.2.1: no raise inside 48h counts as standing pat, which opens the owner's window |
| *sub-event* `RfaMatchWindow` | `deadline_processing::match_or_decline(…, Decline)` | §15.3.2.2: no match inside 48h counts as declining, so the winner signs and forfeits the pick |

## Frontend (commissioner ops console)

A commissioner-only section (gated on `LeagueRole`/admin). Two GraphQL fields are built and no web
page reads either of them yet.

- **Deadline calendar:** the `deadlines(endOfSeasonYear)` query returns this league's `deadline` rows
  (id, date_time, kind, name), oldest first. The passed/upcoming/processed badge still needs a
  `job_run` read, which no query offers. All times shown in CT (see the open questions below).
- **Manual "process now" trigger:** built as `triggerDeadline(deadlineId)`, which calls
  `transaction_processor::process_deadline` and returns the outcome plus the `job_run` id and the
  error detail. It takes the same claim as the scheduler, so a manual run cannot double-process an
  already-`Succeeded` deadline.
- **Job-run status / audit:** a table of `job_run` rows (kind, status, attempts, timestamps). Not
  built: nothing reads the `job_run` table over GraphQL.
- **Error surfacing:** a list of `Failed` job runs with their `error` text and a retry action. Not
  built either; today a commissioner sees a failure only through `triggerDeadline`'s reply or the
  scheduler's logs.

## Edge cases & open questions

- **Timezone:** rules are written in **CT** (§8.2 opening bids Fri 11:59 PM CT, all bids Sun 8:00 PM
  CT; the preseason auction crunch window must open between 8:00 AM and midnight CT — see spec 01's
  timing rules).
  `deadline.date_time` is `DateTimeWithTimeZone`; all comparisons must be tz-aware and the poller must
  use absolute instants. Decide a canonical storage tz (UTC) and render CT in the console; account for
  DST when generating weekly `InSeasonRosterLock` / FA deadlines.
- **Multi-league:** one shared scheduler covers every league (scoped per `deadline.league_id`), and
  `job_run.league_id` keeps the audit rows separate. A league whose oldest due deadline fails holds up
  only its own later deadlines.
- **Replay vs live coexistence with `import-data`:** historical replay already calls the same `logic`
  handlers directly. Replay must **not** create `job_run` rows / must not be picked up by the live
  poller (e.g. only seed `deadline` rows for the *current* live season into the poll window, or mark
  replayed deadlines `Succeeded` up front). Open question: how to fence the boundary between
  replayed-history and live-present so the scheduler only fires future deadlines.
- **Idempotency on re-fire:** the unique `idempotency_key` and the claim to `Running` cover it, and a
  `Running` row untouched for `STALE_RUNNING_TIMEOUT_MINUTES` (10) is reclaimed as an abandoned worker.
  A crash between the handler's commit and its outcome write cannot leave committed work behind a
  `Running` row, because the `Succeeded` write shares the handler's transaction.
- **Manual override:** commissioner "process now" and "retry" must share the idempotency path and write
  `job_run` rows (audit who/when), so manual and automatic processing are indistinguishable downstream.
- **Open:** RD-activation / RDI eligibility guards are missing (`logic/CLAUDE.md` #4); if the processor
  ever drives those automatically, the missing guards become a correctness hole.

## Dependencies

- **Runs the timers of** [spec 01](01-live-auction-engine.md) (auction opens, tier moves and closes)
  and [spec 03](03-rfa-resolution-and-compensation.md) (the two 48h RFA windows).
- **Relates to** spec 06 (the commissioner GraphQL fields this console needs) and spec 08.
- **Soft-depends on** the `server/` team/contract/trade GraphQL resolvers (currently commented out) for
  the frontend console.
