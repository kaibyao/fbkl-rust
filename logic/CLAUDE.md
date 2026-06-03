# logic/ — Rules Engine

This crate holds **all** FBKL fantasy-basketball business logic. The `server/` crate has none —
it only exposes GraphQL + auth and delegates here. When a rule changes, it changes here.

For a full implemented-vs-missing inventory, see `notes/IMPLEMENTED.md`.

## Where rule values live

Roster limits, salary caps, draft config, and counts are **not** hardcoded in this crate —
they live in `constants/src/league_rules/config_settings.rs` (each `///`-documented). Read
values from there; do not duplicate literals into logic.

## Domain map

| Module | Owns |
|--------|------|
| `trade/` | Propose / accept / process trades; asset transfer; external-trade invalidation. |
| `auction/` | FA auctions + preseason veteran auctions; signing winning bids. |
| `annual_contract_advancement/` | PreseasonStart: expire FAs, advance every other contract a year. |
| `deadline_processing/keeper_deadline/` | Validate + record team keepers at the keeper deadline. |
| `deadline_processing/roster_lock/` | Validate rosters (IR/type/cap) and lock them at each lock deadline. |
| `draft_picks/` | Generate future draft picks (rounds 1..=N, N seasons ahead). |
| `drop_contract/` | Drop a contract from a team; record dropped-contract cap penalty data. |
| `ir/` | Move a contract to / activate from IR. |
| `rookie_development_activation/` | Activate an RD/RDI contract into a rookie contract. |
| `rookie_development_international/` | Move contracts RD↔RDI (stateside ↔ international). |
| `roster/` | Salary + cap calculation (incl. dropped-contract penalties). |
| `team_ownership/` | Resolve which team a user owns in a league. |

## Conventions (follow these when adding logic)

1. **Every state change is a transaction + team_update.** A mutation almost always: mutates
   the contract/asset, inserts a row in `transaction` (a `TransactionKind`), and inserts one or
   more `team_update` rows. The transaction is the league's audit log; team_updates drive the
   per-team UI/state. Don't mutate state without recording both.

2. **Wrap multi-step mutations in a DB transaction** — `db.begin()` … `commit()`. Trade and
   auction flows do this; match the pattern.

3. **Delegate persistence to `entity/src/queries/`.** This crate composes query functions; it
   should not build raw SeaORM statements. If a query is missing, add it to `entity`, not here.

4. **Validate eligibility before mutating.** `drop_contract` and `ir` check `ContractStatus` /
   `is_ir` first and `bail!`/`ensure!` on violation. Note: `rookie_development_activation` and
   `rookie_development_international` currently *skip* this guard — that's a known inconsistency,
   not a pattern to copy. Add guards to new mutators.

5. **Contracts form a chain.** Mutations create a *new* contract record (previous/original ids
   linked) rather than editing in place. Validate "latest in chain" before acting on a contract
   in trades (`validate_contract_is_latest_in_chain`).

6. **team_update status convention:**
   - `Done` — applied immediately (advancement, drop, completed trade).
   - `Pending` — recorded now, finalized later by deadline/roster-lock processing (ir, rookie
     activation, RDI moves, auction wins).
   - `InProgress`/`Error` — used by keeper-deadline batch processing.

7. **Effective dates come from deadlines.** Most mutations look up the relevant `deadline` and
   stamp `team_update.effective_date` from it. Some accept an override (`maybe_override_effective_date`).

## Gotchas

- `insert_team_updates_from_completed_trade` **panics** if a team's pre-trade salary is missing
  from the cache — ensure salaries are computed for every involved team before calling.
- Trades validate **asset ownership only** — there is no cap/roster legality check at trade time.
- `end_fa_auction` ignores its `maybe_override_effective_date` param and treats a no-bid auction
  as a hard error (the veteran path expires instead). Don't assume FA and veteran auctions are
  symmetric.
- Future-draft-pick generation failure inside `lock_rosters` is logged and swallowed, not propagated.

## After editing

`cargo build` → `cargo test` → `cargo clippy` → `cargo fmt`. If you touched anything the GraphQL
schema exposes, regenerate frontend types (see root `CLAUDE.md`).
