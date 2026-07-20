# Idiomatic Rust Audit — Fix Plan

Full-workspace audit against the `idiomatic-rust` skill (judgment-level idioms only; clippy pedantic+nursery already passes clean workspace-wide). Scope: all Rust crates including the excluded `import-data`.

**Baseline:** `cargo clippy --workspace` is clean. The one exception is `import-data`, which is excluded from the workspace and has its own stricter `#![deny(clippy::all)]` that is **currently red** (3 errors).

Findings are grouped by theme and ordered by priority. Each item is `path:line — problem → fix`. Phases 1–2 are correctness (real bugs / security), 3–4 are architecture, 5 is polish.

---

## Phase 1 — Real bugs (crash / data-loss on real data)

These are not style — they panic or destroy data on inputs that occur in production.

### Migration
- `migration/src/m20220930_011056_seed_positions.rs:43-45` — `down()` runs `player::Entity::delete_many()` with **no filter**, wiping the entire `player` table on rollback though `up()` only seeded `position` rows. → Scope the delete to seeded rows, or don't touch `player` in `down()` at all.
- `migration/src/m20221112_151717_create_trade_tables.rs:162-167` — `TeamTrade::TeamId` (an FK) marked `.auto_increment()` (copy-paste from the PK column). → Drop `.auto_increment()`.
- `migration/src/m20221112_151717_create_trade_tables.rs:168-173` — same bug on `TeamTrade::TradeId`. → Drop `.auto_increment()`.

### Entity
- `entity/src/entities/draft_pick_option.rs:126-146` — `VALID_BEFORE_AND_AFTER_STATUSES` HashMap omits the real variant `InvalidatedByExternalTrade`; reachable via `validate_update_status`. → Replace HashMap with an exhaustive `match` so every variant must declare transitions at compile time.
- `entity/src/entities/draft_pick_option.rs:176-180` — `.unwrap_or_else(|| panic!(...))` panics in prod once a persisted option has the missing status. → Return `Err(DbErr::Custom(...))` (moot after the match fix above).
- `entity/src/entities/trade.rs:243` — `model.original_trade_id.as_ref().as_ref().unwrap()`: `is_set()` only proves the `ActiveValue` is `Set`, not that the inner `Option<i64>` is `Some`; explicit `None` panics. → Match/handle `Some`/`None`.
- `entity/src/entities/trade.rs:258` — identical unwrap-on-`None` bug in `original_trade_requires_unset_previous_trade`. → Same fix.
- `entity/src/entities/contract/annual_contract_advancement.rs:141,145` — `Ok(1)` used as sentinel for "salary TBD", indistinguishable from a real $1 salary. → Return `Option<i16>`, `None` = TBD.
- `entity/src/entities/contract/annual_contract_advancement.rs:160` — `rounded_up.to_string().parse().unwrap()` round-trips `Decimal`→`String`→`i16`, unwrapping a fallible parse. → `rounded_up.to_i16()` (num_traits `ToPrimitive`), propagate error.

### Logic
- `logic/src/deadline_processing/roster_lock/validate_rosters.rs:177,188,197,221` — `team_contracts[0].team_id.unwrap()` (positional index + unwrap on `Option<i64>`) panics on empty slice / missing team; sibling validator already takes `team_id: i64` explicitly. → Pass `team_id: i64` in, drop the index/unwrap.
- `logic/src/auction/fa_auction.rs:20,42` — `end_fa_auction` accepts `maybe_override_effective_date: Option<NaiveDate>` but never reads it; signature silently lies (crate's own CLAUDE.md flags it). → Wire it in, or drop the param.
- `logic/src/annual_contract_advancement/create_team_contracts_for_annual_advancement.rs:31` — `.expect(...)` on `contract_model.team_id` panics on a data-invariant violation. → `.ok_or_else(|| eyre!(...))?` (or prefix `internal error:` if kept).

### import-data (long-running import aborts mid-run on one bad row)
- `import-data/src/real_world/import_players.rs:197-260` — ~20 chained `.unwrap()` parsing untrusted NBA JSON; one bad field aborts the whole import. → Per-row `Result` with `?`/context, `.collect::<Result<Vec<_>>>()` or skip-and-log.
- `import-data/src/league/transactions/process_seasonal_transactions.rs:110,160-161` — `deadlines[deadline_cursor_index]` indexed right after a loop that can exit with `index == len()`; OOB panic mid-import. → `.get(idx).ok_or_else(...)?`.
- `import-data/src/league/parse_csv/csv_transaction_utils.rs:10-22` — `get_player_name_from_player_csv_str` only `dbg!`s a malformed (<3 token) string, then pops anyway: silently returns `""` for 2 tokens, panics for 0-1. → Return `Result`, `ensure!(len >= 3)`.
- `import-data/src/league/transactions/seasonal_keeper_deadline_contracts.rs:186` — `panic!` inside a `.filter_map()` closure aborts the run. → Restructure to `.map(...) -> Result` + `collect::<Result<Vec<_>>>()`.
- `import-data/src/league/import_original_team_contracts.rs:60` — `csv_contract.salary[1..].parse()?` panics with index-OOB on empty input and duplicates the safer `parse_salary_as_i16()` helper. → Call `parse_salary_as_i16(&csv_contract.salary)?`.
- `import-data/src/league/import_original_team_contracts.rs:115` — `panic!("Unknown contract year")` aborts the loop on one bad row. → `bail!` with row context.
- Scattered parse-then-`unwrap` on external CSV/config (skip-and-log or propagate instead): `import_teams.rs:121`, `import_draft_picks.rs:53`, `parse_draft_picks.rs:57`.

---

## Phase 2 — Security / systemic correctness

### DbErr leaks to GraphQL clients (systemic, high)
Nearly every GraphQL resolver returns bare `Result<T>` (`async_graphql::Error`) and `?`s a raw `DbErr`, which converts via the blanket `From<T: Display>` impl and sends **unredacted internal DB error text to the client** — bypassing `FbklError`'s deliberate redaction/status mapping in `server/src/error.rs`.

Sites: `server/src/graphql/league/league_types.rs:37-47`; `contract/contract_types.rs:83,88`; `team/team_types.rs:55-60,70-77`; `player/player_types.rs:53-62,120-125,131-136`; `league/league_resolvers.rs:65,87,96-108`.

→ Return `Result<T, FbklError>` from resolvers and convert DB errors at the boundary. This is the single most common defect in the audit; fix as one systemic pass.

### Error-swallowing that hides DB outages as "not logged in"
- `server/src/session.rs:23` — `get_current_user` uses `.unwrap_or_default()`, collapsing DB failure + "no user" into `None`. → Return `Result<Option<user::Model>, FbklError>`.
- Downstream of the above: `login_handlers.rs:86`, `league_types.rs:56-58` fold real errors into `None`.

---

## Phase 3 — Error architecture

- **`logic/` has no domain error enum** — the core business-logic crate returns `color_eyre::Result` everywhere with ad-hoc `eyre!`/`bail!` strings; callers can't match on failure kind. → Introduce `LogicError` (`thiserror`) with per-mode variants, convert at boundaries. *(Large; do after Phase 1–2.)*
- **`auth/` leaks dependency error types** — `auth/src/lib.rs:11,29,42,55` return raw `hex::FromHexError`/`argon2::Error`/`argon2::password_hash::Error`, forcing `server` to enumerate auth's dep internals. → Define `AuthError` in the crate with `#[from]`; `FbklError` wraps that one type.
- **Entity query error-type inconsistency** — most query files use `color_eyre::Result`, but `league_queries.rs`, `user_queries.rs`, `user_registration_queries.rs` return raw `Result<T, DbErr>`. → Convert at the boundary for consistency.
- `server/src/session.rs:9` — `enforce_logged_in` returns raw `Result<i64, StatusCode>`. → Return `Result<i64, FbklError>` (the `From<StatusCode>` already exists).
- **Error-text convention** (low, batch): `DbErr::Custom` messages should start with a lowercase verb, not a capitalized sentence — `auction_bid.rs:100`, `team_update.rs:279`, `team_user.rs:135`, `trade.rs:246,261,274`, `contract_entity.rs:494-596`, `transaction_entity.rs:243-301`, `trade_asset.rs:265`; `transaction-processor/src/lib.rs:352-355`.

---

## Phase 4 — Illegal states unrepresentable (type modeling)

Mutually-exclusive `Option<i64>` field groups enforced only by runtime validators. Model each as an enum so invalid combinations can't be constructed. Largest wins first:

- `entity/src/entities/transaction/transaction_entity.rs:32-46` — **7** mutually-exclusive `Option<i64>` fields, each 1:1 with a `TransactionKind` variant. → `TransactionKind::Trade(i64) | AuctionDone(i64) | ...`.
- `entity/src/entities/trade_asset.rs:19-320` — 3 mutually-exclusive target ids gated by `asset_type`. → `enum TradeAssetTarget { Contract(i64), DraftPick(i64), DraftPickOption(i64) }`.
- `entity/src/entities/contract/contract_entity.rs:54-63` — `league_player_id`/`player_id` (exactly-one) + `previous_/original_contract_id` (co-dependent). → Model player-ref + lineage as enums.
- `entity/src/entities/trade.rs:240-280` — trade lineage via two `Option<i64>`. → `enum { Original, Chained { previous, original } }`.
- `entity/src/entities/team_update.rs:272-286` — `transaction_id` valid iff data is `Assets`. → Fold `transaction_id` into the `TeamUpdateData::Assets` variant.
- `entity/src/entities/team_user.rs:129-135` — `final_end_of_season_year` valid iff role `Inactive`. → Carry the year on the `Inactive` variant.
- `entity/src/entities/contract/rookie_draft.rs:31-35` — `is_league_player: bool` gates which `Option` is set. → `enum PlayerRef { Player(i64), LeaguePlayer(i64) }`.
- `entity/src/entities/contract/free_agent_extension.rs:37-61` — declare-then-assign of 3 `mut` fields; `:59` unreachable `bail!` arm. → Single `let (..) = match {..}` expression; narrow param to a 3-kind enum.
- `server/src/graphql/contract/contract_types.rs:20-21` — `Contract` GraphQL type repeats the two-`Option` player-ref illegal state; `LeagueOrRealPlayer` enum already exists in `player.rs`. → Push the enum down into `Contract`.
- `transaction-processor/src/lib.rs:42-124` — `ProcessableEvent`'s 5 variants all carry the identical `{league_id, end_of_season_year, auction_id}`; 4 accessor methods re-match all arms. → `struct ProcessableEvent { <shared fields>, kind: ProcessableEventKind }`.
- `import-data/src/league/transactions/seasonal_trade_transactions.rs:34-86` — `from_owner`/`to_owner` use `""` as "not yet known" sentinel; `get_status()` derives state from `.is_empty()`. → `Option<String>` + match.
- `transaction-processor/src/lib.rs:127-135` — `idempotency_key()` uses `{:?}` (Debug) on the event kind for a DB-persisted uniqueness key; renaming a variant silently changes the format. → Explicit stable `Display`/`as_key_str()`.

---

## Phase 5 — Polish (newtypes, DRY, iterators, minor)

### Swappable same-typed params (newtype ids or object args)
Pervasive adjacent bare `i64`/`String` params with no compiler protection. Highest-value: `FromTeamId`/`ToTeamId` (swap reverses a trade):
- `entity/src/queries/trade_asset_queries.rs:69-73,84-89` and `entity/src/entities/trade_asset.rs:33` — `from_team_id`/`to_team_id`.
- `entity/src/queries/league_queries.rs:12-17` — 3 adjacent `String` params. → `NewLeagueWithCommissioner` struct.
- Others (lower): `rookie_draft_selection.rs:20`, `team_update.rs:77`, `trade_action.rs:25`, `contract_entity.rs:197-213`, `rookie_draft.rs:5-12`, `veteran_auction_contract.rs:5-9`, `team_user_queries.rs:59-63,78-82`, `rookie_draft_selection_queries.rs:33-37`, `auction_queries.rs:34-41,71-77`, `server/.../league_resolvers.rs:71`, `session.rs` id types.
- `logic/src/roster/salary_calculation.rs:30,46,65` — `(i16, i16)` = `(salary, cap)` threaded positionally through 4 call sites. → `struct SalarySnapshot { salary, cap }`.
- `constants/src/lib.rs:5` — `FREE_AGENCY_TEAM: (&str,&str,&str,i32,i16)` indexed `.0`.. at 3 call sites. → named struct.

### DRY
- `import-data` — "find team name for owner at season year" reimplemented ~8× (`import_draft_picks.rs:55-114`, `process_seasonal_transactions.rs:124-140`, `seasonal_keeper_deadline_contracts.rs:156-173`, `seasonal_rookie_selection_transactions.rs:67-84,206-222,426-442`, `import_original_team_contracts.rs:92-106`). One proper fn already exists: `seasonal_trade_transactions.rs:287-318`. → Extract one shared helper, call everywhere.
- `import-data/src/league/import_deadlines.rs:37-187` — 12 near-identical `ActiveModel` push blocks. → `[(accessor, kind, name)]` table iterated once.
- `migration/*` — dead/no-op local transactions (`begin`/`commit` wrapping DDL that runs via `manager`, not the txn handle): `m20220922_012310`, `m20220924_004529`, `m20221023_002183/184/185`. → Delete local txn wrapper (runner is already transactional on Postgres); drop unused `TransactionTrait` imports.

### import-data clippy red (quick)
- `import-data/src/main.rs:1` has `#![deny(clippy::all)]`; currently 3 errors: `useless_borrows_in_formatting` at `import_players.rs:81` (×2), `import_teams.rs:123`. → Remove the redundant `&`s.

### Batch DB inserts (perf, post-profile — mostly low)
- `import_players.rs:70-84` (one-at-a-time insert of hundreds of rows; also `Vec::with_capacity`), `import_teams.rs:95-172`, `import_deadlines.rs:197-201`, `import_owners.rs:126-141,244-272`. → `insert_many` / pre-size vecs.

### Misc low
- `transaction-processor/src/lib.rs:158,176,204` — unused `+ Debug` bound on `C`. → Drop it.
- `logic/src/trade/validate_trade_assets.rs:85-125` — `db`/`C` params never used (only there for `#[instrument]` Debug-format). → Drop.
- `lambdas/src/lib.rs:29` — `.expect(...)` on missing env var panics the Lambda though `init_db` returns `Result`; verbatim dup of `server/src/lib.rs:34`. → `.map_err(|_| DbErr::Custom(...))?`; share the env read.
- Iterator/expression cleanups: `logic/src/trade/propose_trade.rs:46-49`, `accept_trade.rs:57-83`, `draft_picks/future_draft_picks.rs:27-28`; `entity/src/entities/contract/contract_entity.rs:140-145` (`max_by_key` over sort+pop), `annual_contract_advancement.rs:20-127` (build-then-mutate).
- `entity/src/queries/team_update_queries/mod.rs:16-48` — struct + impl defined directly in `mod.rs`. → Move to own file, `pub use`.
- Silent-collision `HashMap` keyed by name: `league_player_queries.rs:19-26`, `team_queries.rs:26-39`. → Return `Vec` unless name uniqueness is DB-enforced.
- `checked_add_days`/date `.expect()` that should propagate: `auction_queries.rs:47,51`, `contract_queries.rs:155`.
- `contract_queries.rs:89` — `is_before_pre_season_keeper_deadline: bool` → enum.
- `import-data/src/constants.rs` + `process_seasonal_transactions.rs:50-51` — `once_cell::sync::Lazy` → std `LazyLock` (edition 2024); drop `once_cell` dep.
- Dead commented code: `server/src/graphql/contract.rs:1-2`, `server/src/graphql/team/team_resolvers.rs:1-38`.
- `graphql-generation/src/main.rs:27-28`, `import_players.rs:317` (no-op rebind), `LazyLock` vs plain `static` inconsistency in `logic/src/roster/salary_calculation.rs:15-16`.

---

## Suggested execution order
1. **Phase 1** — bugs, per crate. Each is a small isolated fix; ship as one PR or a few.
2. **Phase 2** — DbErr redaction pass (one focused PR across the graphql module).
3. **import-data clippy red** — trivial, unblocks that crate's gate.
4. **Phase 3** — error architecture (`LogicError`, `AuthError`, entity consistency). Larger; sequence after correctness.
5. **Phase 4** — type-modeling refactors; biggest first (transaction_entity's 7 fields). These touch migrations/serialization — verify round-trip carefully.
6. **Phase 5** — polish, opportunistically alongside the above.
