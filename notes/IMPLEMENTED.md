# Implemented Feature Inventory

> **Purpose:** A factual inventory of what the FBKL codebase has actually built, to be
> diffed against `notes/2025-08-31-rules_document.md` to determine remaining work.
>
> **Status legend:** ✅ implemented · 🟡 partial / has gaps · 🔴 stub / not built
>
> **Snapshot date:** 2026-06-02. Re-verify against source before trusting; this is a
> point-in-time map, not a contract.

---

## Crate-level summary

| Crate | LOC (src) | Status | Role |
|-------|-----------|--------|------|
| `logic` | ~2,900 | ✅ | Core fantasy-basketball rules engine. Fully implemented across all domains. |
| `entity` | ~6,900 | ✅ | SeaORM models + query functions. Fully implemented. |
| `import-data` | ~5,600 | ✅ | Real-world NBA import + full historical CSV transaction replay (2014-15 →). |
| `constants` | ~120 | ✅ | League rule values (roster limits, salary caps, draft config). Well documented. |
| `auth` | ~85 | ✅ | Argon2 password hashing + auth helpers. |
| `server` | ~1,370 | 🟡 | Axum + GraphQL + session auth. **Only user + league GraphQL is live.** team/player/contract resolvers commented out. |
| `transaction-processor` | ~400 | ✅ | Dispatcher: `process_deadline`/`process_event` run `logic` handlers in one DB txn, idempotently (`job_run` claims), and record outcomes (spec 05). |
| `jobs` | ~100 | 🟡 | Scheduler: `run_scheduler_tick`/`spawn_scheduler` poll due deadlines and dispatch them. Auction/RFA sub-event discovery pending specs 01/03. |
| `migration` | — | ✅ | 15 SeaORM migrations covering all current tables. |
| `graphql-generation` | — | ✅ | Emits GraphQL schema for frontend type generation. |

> The orchestration layer (spec 05) is built: `transaction-processor` dispatches due
> deadlines/sub-events idempotently and `jobs` polls for them inside `fbkl-server`. Sub-event
> *discovery* (auction close timers, RFA windows) still depends on specs 01/03.

---

## logic/ — rules engine (✅ all domains implemented)

No `todo!()`/`unimplemented!()`/empty bodies anywhere in `logic/`. Rule values pulled from
`constants/src/league_rules/config_settings.rs`.

### trade ✅
- `propose_trade` — create a proposed trade (1 team → N teams); inserts `trade`, one `team_trade` per team, the `trade_asset` rows, and a `Propose` `trade_action`.
- `accept_trade` — records an `Accept`; auto-processes once every involved team's latest action is Propose/Accept. Rejects acting on a superseded trade (`validate_trade_is_latest_in_chain`).
- `process_trade` (internal) — moves assets, sets trade `Completed`, inserts a Trade `transaction`, generates per-team `team_update`s, then invalidates conflicting external trades.
- `validate_trade_assets` (internal) — each contract must be latest-in-chain & owned by `from_team`; each draft pick owned by `from_team`; draft pick options must be `Proposed`. At least one asset required.
- `process_trade_assets` (internal) — contracts → `trade_contract_to_team` (new contract record); draft picks → reassign `current_owner_team_id`; options → set `Active`.
- `external_trade_invalidation` — other active trades referencing any just-traded asset (same league + season) set to `InvalidatedByExternalTrade`; affected options invalidated too.
- **Gaps:** No salary-cap / roster-size validation at trade time (validation is asset-ownership only). `insert_team_updates_from_completed_trade` returns an error if a team's pre-trade salary is missing.

### auction ✅ (🟡 minor dead params)
- `start_new_auction_for_nba_player` — inserts an auction for a player contract, generic over `AuctionKind`. *(unused params: `league_id`, `end_of_season_year`; doc comment says "veteran" but is generic.)*
- FA auction (`end_fa_auction`, `get_or_create_player_contract_for_fa_auction`) — ends an FA auction, signs contract to top bidder. **No bid = hard error** (no expiry branch). `maybe_override_effective_date` param is **dead/unused** here.
- Preseason veteran auction (`end_veteran_auction`, `get_or_create_player_contract_for_veteran_auction`) — no bid ⇒ expire contract (player → FA); winning bid ⇒ sign + set team_update effective date. `maybe_override_effective_date` used here. Valid FA types: FreeAgent, RFA, UFA-OriginalTeam, UFA-Veteran.
- `sign_auction_contract_to_team` — signs winning contract, inserts Auction `transaction` + a `team_update` (status Pending, `AddViaAuction`).

### annual_contract_advancement ✅
- `advance_league_contracts` — at PreseasonStart: expire FreeAgent contracts, advance all other kinds; inserts a `PreseasonStart` transaction + per-team `team_update`s (status Done). Requires PreseasonStart deadline.

### deadline_processing ✅
- **keeper_deadline:** `process_keeper_deadline_transaction` (drives team keeper updates: Keeper=no-op, Drop=`drop_contract`), `save_keeper_team_update` (validates + records keepers).
  - Rules: disallow RFA/UFA-OT/UFA-Vet/FreeAgent as keepers. Count limit 14 (`KEEPER_CONTRACT_COUNT_LIMIT`, excludes RD/RDI). Salary limit 100 (`KEEPER_CONTRACT_TOTAL_SALARY_LIMIT`, excludes RD/RDI).
- **roster_lock:** `lock_rosters` (validate rosters, mark deadline team_updates Done; on PreseasonFinalRosterLock also generate future draft picks), `validate_league_rosters` (IR-slot, type-limit, cap checks).
  - Rules: IR slots 0..=1. Preseason: total RD+RDI+vet/rookie ≤ 32. Regular season: RD ≤ 6, RDI ≤ 1, vet/rookie ≤ 22. Cap: total salary ≤ team cap.
  - **Note:** future-draft-pick generation failure propagates (rolls back the lock + fails the job_run).

### draft_picks ✅
- `generate_future_draft_picks` — for each team, rounds 1..=5 (`DRAFT_PICK_ROUNDS`) at `end_of_season_year + 2` (`FUTURE_DRAFT_PICK_SEASONS_LIMIT`); current_owner = original_owner = team.

### drop_contract ✅
- `drop_contract_from_team` — Active contracts only; drops contract (preseason flag from deadline), inserts `TeamUpdateDropContract` transaction (stores `dropped_contract_id`) + Drop team_update (Done). Captures pre/post salary+cap for dropped-contract penalty math.

### ir ✅
- `move_contract_to_ir` (requires `is_ir == false`) / `activate_contract_from_ir` (requires `is_ir == true`). Each creates a `TeamUpdateToIr`/`TeamUpdateFromIr` transaction + a Pending team_update.

### rookie_development_activation ✅ (🟡 no eligibility guard)
- `activate_rookie_development_contract` — activates an RD/RDI contract, inserts `RookieContractActivation` transaction + Pending team_update. **No pre-mutation eligibility validation** (unlike ir/drop).

### rookie_development_international ✅ (🟡 no eligibility guard)
- `move_rookie_development_contract_to_international` (RD→RDI) / `..._to_stateside` (RDI→RD). Each creates ToRdi/FromRdi transaction + Pending team_update. Salary/cap unchanged (neither RD nor RDI counts toward cap). **No eligibility guards.**

### roster ✅
- `calculate_team_contract_salary` (+ `_with_model`, `_at_datetime` wrappers) — sums Rookie/RookieExtension/Veteran salaries (excludes IR). PreseasonKeeper: no penalty. Otherwise: dropped-contract penalty = `ceil(salary * 0.2)` per regular-season dropped cap-counted contract, subtracted from max cap.

### team_ownership ✅
- `get_team_user_access_for_user_in_league` — returns the team where the user's `league_role == TeamOwner`, else None.

---

## entity/ — models + queries (✅ fully implemented)

### Models (24 tables)
auction, auction_bid, deadline, draft_pick, draft_pick_option, draft_pick_draft_pick_option (join), league, league_player, player, position, real_team, rookie_draft_selection, sessions, team, team_trade (join), team_update, team_user, trade, trade_action, trade_asset, user, user_registration, **contract**, **transaction**.

Key enums:
- `ContractKind` { RD, RDI, Rookie, RFA, RookieExtension, UFA-OriginalTeam, Veteran, UFA-FreeAgent, FreeAgent }; `ContractStatus` { Active, Replaced, Expired }.
- `DeadlineKind` (13 deadlines: PreseasonStart, PreseasonKeeper, PreseasonVeteranAuctionStart, PreseasonFaAuctionStart/End, PreseasonRookieDraftStart, PreseasonFinalRosterLock, Week1FreeAgentAuctionStart/End, Week1RosterLock, InSeasonRosterLock, FreeAgentAuctionEnd, TradeDeadlineAndPlayoffStart, SeasonEnd).
- `TransactionKind` (12: Trade, AuctionDone, PreseasonStart, PreseasonKeeper, RookieDraftSelection, TeamUpdateDropContract, TeamUpdateToIr, TeamUpdateFromIr, TeamUpdateToRdi, TeamUpdateFromRdi, RookieContractActivation, TeamUpdateConfigChange).
- `TradeStatus`, `TradeActionType`, `TradeAssetType`, `DraftPickOptionStatus`, `RookieDraftSelectionStatus`, `TeamUpdateStatus`, `LeagueRole`, `PlayerStatus`, `UserAppAdminStatus`, `UserRegistrationStatus`.
- Rich `team_update` JSON payload enums: `TeamUpdateData`, `TeamUpdateAsset`, `ContractUpdateType`, `DraftPickUpdateType`.

Contract transition helpers (pure ActiveModel fns, not tables): advancement, dropped, expire, sign RFA/UFA, rookie-from-rd, rd-from-rdi, from-rookie-draft, trade-to-team, for-auction, sign-veteran.

### Query modules (all ✅)
auction, contract (19 fns incl. chain validation + dropped-contract lookups), deadline, draft_pick, league_player, league, player, position, real_team, rookie_draft_selection, team, team_update (basic/keeper/auction submodules), team_user, trade_action, trade_asset, trade, transaction (incl. get_or_create keeper-deadline transaction), user, user_registration.

---

## import-data/ — data import (✅ fully implemented)

CLI (`ImportDataType`): RealWorld, LeagueTeams, Deadlines, Owners, DraftPicks, OriginalTeamContracts, Transactions, AllLeagueData (+ optional `--year`).

- **Real-world:** import NBA real_teams (bundled JSON) + players (`data/nba_player_index_2025-07-01.json`) → player/position/real_team. `panic!` guards on unexpected data shape.
- **League:** league+teams, deadlines, owners (users + team_users), draft picks, 2014 offseason original contracts + advancement, historical transactions (from 2014-15).
- **CSV transaction replay** — every `CsvTransactionType` handled (no stub arms): Auction, Draft, Drop, FreeAgent, Ir, IrActivate, Keeper, RdActivate, Rdi, TradeAway, TradeFor. Modules: process_seasonal_transactions (dispatch), process_transaction_deadlines, process_drop_contract, process_ir, process_rdi, process_rookie_contract_activation, seasonal_fa_auction, seasonal_veteran_auction, seasonal_keeper_deadline_contracts, seasonal_rookie_selection_transactions, seasonal_trade_transactions (reconstructs multi-asset trades from paired TradeAway/TradeFor rows), validate_transactions (fail-fast guards).

---

## server/ — GraphQL API (🟡 thin / partial)

Root schema (`graphql.rs`): `QueryRoot(UserQuery, LeagueQuery)`, `MutationRoot(LeagueMutation)`. **Only user + league domains are wired in.**

| Domain | Status | Notes |
|--------|--------|-------|
| user | ✅ | Query `currentUser → User { id, email }`. |
| league | ✅ | Queries `leagues`, `league`; mutations `createLeague`, `selectLeague` (session). |
| team | 🔴 | `TeamQuery` resolvers **commented out**, not registered in root. Types exist + used by league resolver (`Team`, `TeamSalaryCap`, `TeamUser`). |
| player | 🔴 | Types only (`LeagueOrRealPlayer`, `LeaguePlayer`, `RealPlayer`); no resolvers. Some type fields commented out. |
| contract | 🔴 | Type `Contract` only; resolver module commented out. |

Handlers (✅): login (login_page/process_login/logout/logged_in_data), user_registration (page/process/confirm), graphql (process_graphql/graphiql), public (get_public_page).

**`server/src/main.rs:87-112`** — ~30 roadmap TODO comments (transaction-processor job, draft-pick/trade GraphQL, NBA API sync, roster legalization, SendGrid email, CSP header, etc.). Aspirational, not in-code stubs.

---

## Known not-built (🔴) — prime suspects for rules-doc gaps

1. **Auction/RFA sub-event discovery in `jobs`** — the scheduler processes `deadline` rows; synthesizing auction-close timers (spec 01) and RFA windows (spec 03) is pending those engines.
2. **GraphQL read/write surface for team, player, contract, trade, draft picks** — commented out or absent. The frontend cannot drive most league operations through the API yet (incl. the spec-05 commissioner ops console).
4. **Trade-time cap/roster validation** — trades validate ownership only, not legality.
5. **Email integration, NBA live sync, roster legalization, CSP** — listed as main.rs TODOs.

---

## How to use this for the rules-doc comparison

1. Read `notes/2025-08-31-rules_document.md` section by section.
2. For each rule, locate the matching domain above and mark: built ✅ / partial 🟡 / missing 🔴.
3. Cross-check rule *values* (limits, caps, counts) against `constants/src/league_rules/config_settings.rs` — they may exist in code but disagree with the doc.
4. Pay special attention to anything requiring **GraphQL mutations beyond user/league** — that layer is not built. Background jobs / transaction processing exist (spec 05) but only for deadline-table events so far.
