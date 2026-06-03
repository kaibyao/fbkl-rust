# Spec 06 — GraphQL API Surface

**Rules ref:** cross-cutting (exposes all engines) · **Status:** 🔴 only user+league live · **Priority:** P0

## Summary

The GraphQL surface in `server/src/graphql.rs` wires only `QueryRoot(UserQuery, LeagueQuery)` and
`MutationRoot(LeagueMutation)`. Everything else is dark: `team` resolvers are commented out
(`team/team_resolvers.rs`), `player` and `contract` are types-only (no Query/Mutation root). Result:
the frontend can list leagues and select one, but cannot drive a single league operation — no roster
view by team, no trades, no auctions, no draft, no keepers. This spec defines the **full** read/write
surface needed to operate the league, mapping every operation to the `logic/` fn it delegates to and
the `LeagueRole` that may invoke it. `server/` stays logic-free: resolvers parse args, resolve the
caller's role from session, call a `logic::` fn, map the model to a GraphQL type. Where a logic fn does
not yet exist (auction bids, draft picks, RFA resolution, deadline triggers), the row points at the
spec that builds it — this spec is the API contract those specs satisfy, not a mandate to build engines
here.

## Backend

### Wiring (register domain roots in `graphql.rs`)

Today (`server/src/graphql.rs`):
```rust
#[derive(Default, MergedObject)]
pub struct QueryRoot(UserQuery, LeagueQuery);
#[derive(Default, MergedObject)]
pub struct MutationRoot(LeagueMutation);
```

Target — each domain exports a `XQuery` and (where it mutates) `XMutation` `#[Object]`, merged in:
```rust
#[derive(Default, MergedObject)]
pub struct QueryRoot(
    UserQuery, LeagueQuery, TeamQuery, PlayerQuery, ContractQuery,
    TradeQuery, DraftQuery, AuctionQuery, RfaQuery, KeeperQuery,
    DeadlineQuery, TransactionQuery,
);
#[derive(Default, MergedObject)]
pub struct MutationRoot(
    LeagueMutation, TradeMutation, DraftMutation, AuctionMutation,
    RfaMutation, KeeperMutation, DeadlineMutation, /* TeamMutation if IR/drop/RD live here */
);
```

Per-domain module layout follows the existing convention (`team.rs` → `team/team_resolvers.rs` +
`team/team_types.rs`). New modules to add under `server/src/graphql/`: `trade/`, `draft/`, `auction/`,
`rfa/`, `keeper/`, `deadline/`, `transaction/`. Uncomment + finish `team/team_resolvers.rs`; add a
`player/player_resolvers.rs` and `contract/contract_resolvers.rs` (both currently absent — `player.rs`
and `contract.rs` only re-export `*_types`).

**Pre-req fixups already needed by the commented `TeamQuery`:** it references `find_teams_by_user` and
`find_team_by_user` (do not exist in `entity/team_queries` — only `find_teams_in_league` does) and a
`selected_team_id` session key (only `selected_league_id` is set today). Either add those query fns to
`entity/` or rewrite `TeamQuery` against `find_teams_in_league(league_id)` + a `team(id)` lookup scoped
to the session's selected league. Prefer the latter — team selection should derive from the active
league, not a second session key.

### Per-domain query/mutation table (operation → logic fn → authz role)

Legend for authz: **Owner** = `LeagueRole::TeamOwner` of the team in question · **Comm** =
`LeagueRole::LeagueCommissioner` · **Member** = any non-`Inactive` role in the selected league ·
**Admin** = `user.app_admin_status` (app-level, not league role).

#### team — UNCOMMENT + complete (`team/`)

| Operation | Kind | Delegates to | Authz |
|---|---|---|---|
| `teams` | Q | `team_queries::find_teams_in_league(selected_league_id)` | Member |
| `team(id)` | Q | `find_teams_in_league` filtered, or add `find_team_by_id` scoped to league | Member |
| `Team.contracts` | Q (field) | `contract_queries::find_active_contracts_for_team` (already wired in `team_types.rs`) | Member |
| `Team.salaryCap(datetime)` | Q (field) | `logic::roster::calculate_team_contract_salary_at_datetime` (already wired) | Member |
| `Team.teamUsers` | Q (field) | `team_user_queries::get_team_users_by_team` (already wired) | Member |
| `Team.teamUpdates(status?)` | Q (field) | `team_update_queries::*` (basic submodule) | Owner(self)/Comm |

No team **mutations** in this spec — IR/drop/RD activation are roster ops; place them under their own
`TeamMutation` (or a `RosterMutation`) so trade/auction stay clean. See roster rows below.

#### player (`player/`) — add resolver root

| Operation | Kind | Delegates to | Authz |
|---|---|---|---|
| `searchPlayers(query, kind?)` | Q | `player_queries` / `league_player_queries` name search (add a `search_*` fn) | Member |
| `realPlayer(id)` | Q | `player_queries::find_player_by_id` | Member |
| `leaguePlayer(id)` | Q | `league_player_queries::find_league_player_by_id` | Member |
| `LeaguePlayer.realPlayer` | Q (field) | `find_player_by_id` (already wired in `player_types.rs`) | Member |
| `playerEligibility(leaguePlayerId)` | Q | derived: `is_rdi_eligible` + active-contract kind checks (read-only; reuse logic guards once spec 02/03 add them) | Member |

Returns `LeagueOrRealPlayer` union (exists). `playerEligibility` is a read-only projection — do **not**
duplicate eligibility rules in the resolver; once specs 02/03 add eligibility guard fns in `logic/`,
have the resolver call those in a dry-run/check mode.

#### contract (`contract/`) — un-comment resolver module, add root

| Operation | Kind | Delegates to | Authz |
|---|---|---|---|
| `contract(id)` | Q | `contract_queries::find_contract_by_id` | Member |
| `Contract.leagueOrRealPlayer` | Q (field) | already wired in `contract_types.rs` | Member |
| `contractChain(contractId)` | Q | `contract_queries` chain-walk fns (the 19-fn module incl. chain validation / `find_*_in_chain`) | Member |

Read-only. Contract mutation happens only as a side effect of trade/auction/keeper/roster ops — never
expose a raw "edit contract" mutation.

#### trade (`trade/`) — new (logic mostly exists)

| Operation | Kind | Delegates to | Authz |
|---|---|---|---|
| `proposedTrades` | Q | `trade_queries` (proposed by/to caller's team) | Owner/Comm |
| `activeTrades` | Q | `trade_queries` (status filter) | Member |
| `trade(id)` | Q | `trade_queries::find_trade_by_id` + `team_trade`/`trade_asset`/`trade_action` joins | Member |
| `proposeTrade(input)` | M | `logic::trade::propose_trade` | Owner (of `from_team`) |
| `acceptTrade(tradeId)` | M | `logic::trade::accept_trade` (auto-processes when all latest actions are Propose/Accept) | Owner (of an involved team) |
| `rejectTrade(tradeId)` | M | add `logic::trade::reject_trade` (insert `Reject` `trade_action`) — **not yet present** | Owner (of an involved team) |
| (conditional pick options) | M field of `proposeTrade` input | options carried as `trade_asset` of type DraftPickOption; `validate_trade_assets` requires status `Proposed` | Owner |

`proposeTrade` input: `fromTeamId`, list of `{toTeamId, assets:[{type, contractId|draftPickId|optionId}]}`.
**Caveat (from IMPLEMENTED.md):** trade validation is ownership-only — no cap/roster legality check, and
`insert_team_updates_from_completed_trade` panics if a team's pre-trade salary is missing. Resolver should
surface that as a clean GraphQL error, not a 500. Cap/roster trade-legality is out of scope here (separate
gap).

#### roster ops (IR / drop / rookie-dev) — `TeamMutation` or `RosterMutation`, new

| Operation | Kind | Delegates to | Authz |
|---|---|---|---|
| `moveContractToIr(contractId)` | M | `logic::ir::move_contract_to_ir` (requires `is_ir==false`) | Owner |
| `activateContractFromIr(contractId)` | M | `logic::ir::activate_contract_from_ir` | Owner |
| `dropContract(contractId)` | M | `logic::drop_contract::drop_contract_from_team` | Owner |
| `activateRookieContract(contractId)` | M | `logic::rookie_development_activation::activate_rookie_development_contract` (⚠ no eligibility guard in logic) | Owner |
| `moveRookieToInternational(contractId)` | M | `logic::rookie_development_international::move_rookie_development_contract_to_international` | Owner |
| `moveRookieToStateside(contractId)` | M | `..._to_stateside` | Owner |

#### draft (`draft/`) — new; **logic mostly NOT built** (see spec 02)

| Operation | Kind | Delegates to | Authz |
|---|---|---|---|
| `draftBoard` | Q | `draft_pick_queries` + `rookie_draft_selection_queries` (selections so far) | Member |
| `draftOrder` | Q | `draft_pick_queries` ordered by round/owner | Member |
| `lotteryResults` | Q | spec 02 lottery output (TBD) | Member |
| `makePick(input)` | M | spec 02 `logic::rookie_draft::make_selection` (**not yet present** — inserts `rookie_draft_selection` + `RookieDraftSelection` transaction) | Owner (on the clock) |
| `passPick(pickId)` | M | spec 02 pass/skip handling (TBD) | Owner (on the clock) |

`generate_future_draft_picks` (exists) is invoked by roster-lock processing, not a GraphQL mutation.

#### auction (`auction/`) — new; **bidding NOT built** (see spec 01)

| Operation | Kind | Delegates to | Authz |
|---|---|---|---|
| `openAuctions` | Q | `auction_queries` (open/active in selected league) | Member |
| `auction(id)` | Q | `auction_queries::find_auction_by_id` | Member |
| `bidHistory(auctionId)` | Q | `auction_bid` queries ordered by amount/time | Member |
| `auctionSchedule` | Q | `deadline_queries` (auction-start/end deadlines) | Member |
| `placeBid(auctionId, amount)` | M | spec 01 `logic::auction::place_bid` (**not yet present** — must enforce min increment, cap, anti-snipe extension) | Owner |
| (auction settlement) | — | `logic::auction::end_fa_auction` / `end_veteran_auction` (exist) run via deadline processing, **not** a direct mutation | Comm/job |

`start_new_auction_for_nba_player` (exists) — exposing a manual "open auction" mutation is optional and
Comm-only; normally auctions open via deadline processing (spec 05).

#### rfa (`rfa/`) — new; **logic NOT built** (see spec 03)

| Operation | Kind | Delegates to | Authz |
|---|---|---|---|
| `rfaResolutions` | Q | spec 03 RFA state queries (open RFA situations + status) | Member |
| `rfaResolution(leaguePlayerId)` | Q | spec 03 | Member |
| `raiseRfa(input)` | M | spec 03 `logic::rfa::submit_raise` (**not built**) | Owner (original team or challenger per rules) |
| `matchRfa(rfaId)` | M | spec 03 `logic::rfa::match_offer` (**not built**) | Owner (original team) |
| `declineRfa(rfaId)` | M | spec 03 `logic::rfa::decline` (**not built**) | Owner (original team) |

RFA re-sign salary (discount caps) comes from spec 04 — the resolver must surface a backend-computed
projected salary, never recompute discount math client-side.

#### keeper (`keeper/`) — new (logic mostly exists)

| Operation | Kind | Delegates to | Authz |
|---|---|---|---|
| `keeperDeclarations(teamId?)` | Q | `team_update_queries` keeper submodule | Owner(self)/Comm |
| `declareKeepers(input)` | M | `logic::deadline_processing::keeper_deadline::save_keeper_team_update` (validates: no RFA/UFA-OT/UFA-Vet/FA; ≤14 contracts excl RD/RDI; ≤100 salary excl RD/RDI) | Owner |
| `validateKeepers(input)` | Q | dry-run of the same validation (return errors without persisting) — factor the validation out of `save_keeper_team_update` if not already callable read-only | Owner |

`process_keeper_deadline_transaction` (exists) is deadline-driven, not a mutation.

#### deadline (`deadline/`) — new

| Operation | Kind | Delegates to | Authz |
|---|---|---|---|
| `deadlines` / `deadlineCalendar` | Q | `deadline_queries` (all 13 `DeadlineKind` rows for the league) | Member |
| `triggerDeadline(deadlineId)` | M | spec 05 deadline-processing dispatch (`lock_rosters`, `advance_league_contracts`, auction-end, keeper processing, etc. by `DeadlineKind`) | **Comm** (or Admin) |

`triggerDeadline` is the commissioner manual-fire path; spec 05 builds the scheduler/job that fires these
automatically. Both call the same `logic::deadline_processing` entry points.

#### transaction (`transaction/`) — new (read-only audit feed)

| Operation | Kind | Delegates to | Authz |
|---|---|---|---|
| `transactions(filter, page)` | Q | `transaction_queries` (by league/team/kind, paginated) | Member |
| `transaction(id)` | Q | `transaction_queries::find_transaction_by_id` | Member |

Covers all 12 `TransactionKind` rows. This is the league history/audit feed; no mutations (transactions
are written only as side effects of the engines above).

### Subscriptions? (real-time auction bids / draft clock)

`FbklSchema` is `Schema<QueryRoot, MutationRoot, EmptySubscription>` today. **Recommendation: defer
async-graphql subscriptions; ship polling first.**

- Auction bids and the draft clock are the only genuinely real-time surfaces. async-graphql subscriptions
  require a WS transport wired into Axum, a broker/channel to fan out events from the bid/pick mutations,
  and ur. client `subscriptionExchange` plumbing — non-trivial, and the league is low-concurrency
  (one league, ~12 teams).
- **Phase 1:** `openAuctions`/`bidHistory` and `draftBoard` polled by urql on a short interval (e.g. 2–5s)
  while a view is open. Anti-snipe extension (spec 01) already lives server-side, so a slightly stale
  client clock is safe.
- **Phase 2 (optional):** add a single `Subscription` root (`auctionUpdated(auctionId)`,
  `draftClockTick(draftId)`) backed by a `tokio::sync::broadcast` channel that `placeBid`/`makePick`
  publish to. Swap `EmptySubscription` → `SubscriptionRoot` and add the WS route. Treat as a follow-up,
  not a blocker for the P0 surface.

### Authz (resolver guards by `LeagueRole`; session-based)

- Auth is session-based: `user_id` and `selected_league_id` live in the `tower_sessions::Session`.
  `server/src/session.rs` provides `enforce_logged_in(session) -> i64` and
  `get_current_user(session, db) -> Option<user::Model>`. The schema injects `Option<user::Model>` and
  `Session` into `Context` (see `league_resolvers.rs` patterns).
- Add a shared guard helper, e.g. `session::require_league_role(ctx, min_role) -> Result<(team_user, team)>`
  built on `team_user_queries::get_team_user_by_user_and_league(user_id, selected_league_id, db)` (already
  used by `League.current_team_user`). It returns the caller's `LeagueRole`; reject `Inactive`.
- Map the table's authz column to guards:
  - **Member**: logged in + non-Inactive `team_user` in the selected league.
  - **Owner**: caller's `team_user.team_id` must equal the team owning the asset (contract/team/auction
    bid). Resolvers that mutate a team's roster must verify the target contract/team belongs to the
    caller's team — do **not** trust client-supplied `teamId` alone.
  - **Comm**: `league_role == LeagueCommissioner` (deadline trigger, manual auction open).
  - **Admin**: `user.app_admin_status` — reserve for cross-league/maintenance, not normal play.
- Prefer async-graphql `Guard` impls over ad-hoc checks inside each resolver so the role requirement is
  declarative and shows up consistently. Keep the actual ownership comparison (caller-team == asset-team)
  inside the resolver since it needs the resolved asset.

### Schema regeneration step (graphql-generation → frontend codegen)

Every added/changed resolver changes the SDL. Pipeline:
1. `cargo run -p fbkl-graphql-generation` (or `cd graphql-generation && cargo run`) — re-emits
   `graphql-generation/generated/fbkl-schema.graphql` from `QueryRoot`/`MutationRoot` (it builds the same
   `Schema` and dumps `.sdl()`). This MUST run after wiring new roots, or the new types won't appear.
2. `pnpm --filter "@fbkl/webapp-logged-in" graphql` — GraphQL Code Generator reads that SDL + the app's
   `.graphql` operation documents to regenerate typed urql hooks.
3. Commit the regenerated SDL + generated TS together with the resolver change so frontend/backend stay
   in lockstep. (Lefthook runs clippy/fmt + tsc/eslint; tsc will fail loudly if generated types drift.)

## Frontend (urql client)

### Generated hooks usage

- Each operation gets a co-located `.graphql` document → codegen emits `useXQuery` / `useXMutation`
  typed hooks (same flow that already powers `leagues`/`createLeague`). Components import the generated
  hook; never hand-write the query string.
- Cross-cutting `selected_league_id` is server-side session state, so most queries take no league arg —
  the resolver reads it from session. Keep that contract: don't add a `leagueId` arg to every field.

### Optimistic updates

- **Trades/keepers/RFA:** these are multi-step and validation-heavy (cap, chain-latest, keeper limits) —
  do **not** optimistically apply; show pending state and reconcile on the server response.
- **Roster toggles (IR move/activate):** safe to optimistically flip `Contract.isIr`, rollback on error.
- **Auction bids:** show the bid as "pending" locally but treat the server's `bidHistory` poll as truth
  (server enforces min-increment / cap / anti-snipe — client guesses can be rejected).

### Cache invalidation strategy

- Use urql Graphcache (normalized) keyed by `id`. After mutations, invalidate the affected entities:
  - `proposeTrade`/`acceptTrade`/`rejectTrade` → invalidate `proposedTrades`, `activeTrades`, both teams'
    `Team.contracts` + `Team.salaryCap`, and `transactions`.
  - roster ops → invalidate that `Team`'s `contracts`/`salaryCap` and `transactions`.
  - `declareKeepers` → invalidate `keeperDeclarations` + team contracts.
  - `placeBid` → invalidate `auction(id)` + `bidHistory`.
- Because `salaryCap` takes a `datetime` arg, ensure Graphcache treats it as args-keyed so different
  datetimes don't collide.

### Real-time approach

- Phase 1: `useQuery(..., { requestPolicy: 'cache-and-network' })` + a polling interval on the auction/
  draft views (urql `pause`/manual `reexecuteQuery` on a timer, or `pollInterval` via wrapper). Tear
  down on unmount.
- Phase 2 (if subscriptions land): add urql `subscriptionExchange` over WS for `auctionUpdated` /
  `draftClockTick` and drop polling on those two views.

## Edge cases & open questions

- **N+1 / DataLoader:** `Team.contracts` → `Contract.leagueOrRealPlayer` → player lookups is a classic
  N+1 (each contract independently calls `find_player_by_id`). Add async-graphql `DataLoader`s for
  `player`, `league_player`, `position`, `real_team` (these are the per-field DB hits in
  `contract_types.rs`/`player_types.rs`). Without it, a full roster view is dozens of round-trips.
- **Pagination for transaction log:** the audit feed can be large (full historical replay from 2014-15).
  Use cursor/offset pagination (`transactions(first, after)` or `(page, pageSize)`) — don't return the
  whole league history unbounded. Pick one convention and apply it to `bidHistory` too.
- **Error shape:** existing resolvers map `StatusCode` → `GraphQlError` (`league_resolvers.rs`) and use
  custom `FbklError` (`server/src/error.rs`). Standardize: domain-validation failures (keeper-limit
  exceeded, not-latest-in-chain, bid-too-low, not-on-the-clock) should be typed GraphQL errors with a
  stable `code` extension the frontend can switch on — not bare 500s. The known panic in
  `insert_team_updates_from_completed_trade` (missing pre-trade salary) must be caught/guarded before it
  reaches the resolver.
- **Owner vs asset ownership:** several mutations take a `teamId`/`contractId`; the resolver must
  re-derive ownership from the DB (caller's `team_user.team_id` == asset's team), never trust the arg.
- **Manual vs job-driven settlement:** auction-end, contract advancement, roster lock, keeper processing
  all have existing `logic` entry points but are meant to fire from deadline processing (spec 05). Decide
  whether Comm-triggered `triggerDeadline` and the scheduler share one dispatch fn (recommended) so manual
  and automatic paths can't diverge.

## Dependencies

- **Foundational for** specs 01 (auction `placeBid`), 02 (draft `makePick`/`passPick`), 03 (RFA
  raise/match/decline), 05 (`triggerDeadline`), 07/08 — this spec is the API contract those engines plug
  into.
- **Depends on** those same specs for the *signatures* of not-yet-built logic fns
  (`logic::auction::place_bid`, `logic::rookie_draft::make_selection`, `logic::rfa::*`, the deadline
  dispatch). Land the read surface (team/player/contract/transaction Q + trade/keeper/roster M, all
  backed by existing logic) first; wire the engine mutations as each engine spec completes.
- **Depends on** spec 04 for backend-computed RFA/UFA re-sign salaries surfaced via `rfaResolution`.
