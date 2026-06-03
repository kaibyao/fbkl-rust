# Spec 10 — Eligibility & Player Pool
**Rules ref:** §3, §6.2, §7.5, §8.4, §11 · **Status:** 🔴 classification + guards missing · **Priority:** P1

## Summary

The whole league splits players into two acquisition pools by one fact: **has the player ever
been on an active NBA roster?** (§3.1.2 — defined by an NBA entry on basketball-reference.com;
game minutes not required). That single pivot decides:

- **Veteran Auction pool** (§6.2.1): all players who *have* been on an active NBA roster.
- **Rookie Draft pool** (§7.5): a constrained set of players who have *never* been on an active
  NBA roster.
- **In-season FA pool** (§8.4): the union of the two above; everyone ineligible for both auction
  and draft stays ineligible for FA.

This pivot is **not modeled today.** `player`/`league_player` carry only `is_rdi_eligible: bool`
and (on `player`) `PlayerStatus { Active, Retired }` — neither encodes NBA-roster history nor a
draft-vs-auction classification. Spec covers: (1) an eligibility model on the player entities,
(2) where the NBA-roster fact is ingested from (cross-ref [spec 12](12-out-of-scope-and-external.md)),
(3) pool-assembly functions in `logic/`, (4) the **missing eligibility guards** on
`rookie_development_activation` + `rookie_development_international` (a known gap — they skip the
pre-mutation `ensure!`/`bail!` that `ir`/`drop` enforce), (5) an RDI eligibility validator, and
(6) a commissioner override for the §3.1.2 / §11.3.6 "decided by the commissioner" edge cases.

NBA-roster status and NBA-IR status are derived/ingested data — see
[spec 12](12-out-of-scope-and-external.md) for the source and freshness contract.

## Backend

### Player eligibility model (entity)

Add to **`player`** (the real-world NBA player; `entity/src/entities/player.rs`) and mirror on
**`league_player`** (`entity/src/entities/league_player.rs`, used for drafted-but-not-yet-NBA
custom players) the following fields, via a new migration:

- `has_been_on_nba_roster: bool` — the §3.1.2 pivot. Source of truth for the auction-vs-draft
  split. On `league_player`, `false` is the normal case (a custom player exists *because* he has
  no NBA entry yet); flipping to `true` is the §11.3.1 trigger to move RDI→RD/1.
- `nba_roster_source: NbaRosterSource` enum `{ BasketballReference, Espn, Nba, CommissionerOverride, Unknown }`
  — provenance of the flag.
- `nba_roster_asof: Option<DateTimeWithTimeZone>` — when the flag was last evaluated (freshness;
  the flag is only as good as the last ingest — see edge cases).
- `eligibility_override: Option<EligibilityClassification>` — commissioner manual override
  (§3.1.2, §11.3.6). When `Some`, it wins over the derived classification.
- `eligibility_override_reason: Option<String>` + `eligibility_override_by_team_user_id` /
  `..._at` — audit trail for the override.

New enum (entity, `async_graphql::Enum` + `DeriveActiveEnum`, same pattern as `PlayerStatus`):

```rust
pub enum EligibilityClassification {
    RookieDraftEligible,    // never on NBA roster AND in the §7.5.1 eligible set
    VeteranAuctionEligible, // has been on an active NBA roster (§6.2.1)
    Ineligible,             // §7.5.2 / §8.4.2 — current college/HS, undrafted foreign non-collegian, etc.
}
```

Classification is **derived**, not stored as a column, by a pure fn (see pool assembly). The
stored `eligibility_override` short-circuits it. Reasoning for keeping it derived: `RookieDraft`
vs `Veteran` membership shifts over time (a draft-eligible player who signs an NBA contract
mid-cycle flips to veteran — §11.3.1) and must always reflect current `has_been_on_nba_roster`.

`PlayerStatus { Active, Retired }` stays as-is — it answers "show in search?", a different
question than "which pool?". Do not overload it.

### Ingestion (where NBA-roster status comes from)

`import-data/src/real_world/import_players.rs:312` currently sets `PlayerStatus` from
`player.to_year == 2024` only — it derives nothing about NBA-roster history. Extend the importer
(NBA player index `data/nba_player_index_*.json`; cross-ref [spec 12](12-out-of-scope-and-external.md)):

- A player present in the NBA player index with any season played ⇒ `has_been_on_nba_roster = true`,
  `nba_roster_source = Nba` (or `BasketballReference` once that source is wired), `nba_roster_asof = now()`.
- A `league_player` (drafted, no NBA entry) ⇒ `has_been_on_nba_roster = false`, source `Unknown`
  until an ingest confirms, asof set on each run.
- **Manual / commissioner path:** a mutation to set `has_been_on_nba_roster` +
  `nba_roster_source = CommissionerOverride` for the §3.1.2 "any further questions … decided by
  the commissioner" cases and for players the automated feed lags on (summer-league signings,
  10-day contracts). This is distinct from `eligibility_override` (which overrides the *derived
  classification*, not the underlying fact) — prefer correcting the fact when the issue is a stale
  feed, and use `eligibility_override` only for genuine judgment calls.

The basketball-reference source itself (scrape/feed) is out of scope here — owned by
[spec 12](12-out-of-scope-and-external.md). This spec consumes whatever fact spec 12 lands.

### Pool assembly fns (logic)

New module `logic/src/eligibility/` exposing pure classifiers + pool builders. These compose
`entity` queries; they do not build raw SeaORM (per `logic/CLAUDE.md` conv. 3). Add the
underlying filtered queries to `entity/src/queries/player_queries.rs` /
`league_player_queries.rs` (currently only `find_player_by_id` / `find_players_by_name`).

- `classify_player(player_facts) -> EligibilityClassification` — pure. Order:
  1. If `eligibility_override.is_some()` ⇒ return it.
  2. If `has_been_on_nba_roster` ⇒ `VeteranAuctionEligible` (§6.2.1).
  3. Else if in the §7.5.1 rookie-eligible set (drafted-this-year, declared-undrafted,
     summer-league-never-NBA, G-League-never-NBA, previously-drafted-foreign-never-NBA,
     former-American-collegian-overseas-never-NBA) ⇒ `RookieDraftEligible`.
  4. Else (§7.5.2: current college/HS, undrafted foreign non-collegian, other) ⇒ `Ineligible`.
  - Note: the §7.5.1 sub-categories are not yet representable from current data (we only have
    "in NBA index" vs "custom league_player"). Until [spec 12](12-out-of-scope-and-external.md)
    or [spec 02](02-rookie-draft-engine.md) lands richer source tags, approximate
    `RookieDraftEligible = !has_been_on_nba_roster && (is a drafted league_player || flagged
    draft-eligible)` and rely on `eligibility_override` for edge cases. Capture the data gap as
    an open question below.
- `build_veteran_auction_pool(league_id, end_of_season_year, db)` — players classified
  `VeteranAuctionEligible` that are not currently rostered keepers, partitioned into FA / UFA /
  RFA per §6.2.2 by reading keeper outcomes (cross-ref [spec 01](01-live-auction-engine.md)
  auction pool + the keeper-deadline results in `deadline_processing/keeper_deadline`).
- `build_rookie_draft_eligible_pool(league_id, end_of_season_year, db)` — players classified
  `RookieDraftEligible`. **§7.5.3:** prior league draft/ownership does **not** affect eligibility —
  a previously-drafted-but-now-unrostered, never-NBA player is still eligible. So the filter keys
  off classification + current-roster status only, never historical `contract` rows. (Caveat the
  §7.3.4 same-draft re-draft rule — that's a draft-engine concern, [spec 02](02-rookie-draft-engine.md).)
- `build_in_season_fa_pool(league_id, end_of_season_year, db)` — `VeteranAuctionEligible` ∪
  `RookieDraftEligible` minus currently-rostered (§8.4.1). `Ineligible` players stay out (§8.4.2).
  Dropped players re-enter with their pre-drop salary as minimum bid — that minimum-bid logic is
  [spec 01](01-live-auction-engine.md); this fn only governs *membership*.

### Add missing eligibility GUARDS to rookie_development_activation + rookie_development_international

Per `logic/CLAUDE.md` conv. 4, `drop_contract` and `ir` `ensure!`/`bail!` on state before
mutating (e.g. `ir/move_contract_to_ir.rs:68`). These two modules skip that and must be brought
in line:

- **`logic/src/rookie_development_activation/activate_rookie.rs`** —
  `activate_rookie_development_contract` mutates with no pre-checks. Add, before computing salary:
  - `ensure!` the contract's `ContractKind` is `RD` or `RDI` (cannot activate a non-RD/RDI).
  - `ensure!` the contract is `ContractStatus::Active` and latest-in-chain
    (`validate_contract_is_latest_in_chain`), matching trade/drop guards.
  - `bail!` with a clear message otherwise — don't silently produce an invalid `RookieContractActivation`.
- **`logic/src/rookie_development_international/move_rd_contract_to_international.rs`**
  (`move_rookie_development_contract_to_international`, RD→RDI) — guard:
  - `ensure!` source kind is `RD` (not already RDI, not R/V).
  - `ensure!` the player passes the **RDI eligibility validator** below.
- **`logic/src/rookie_development_international/move_rdi_contract_from_international.rs`**
  (`move_rookie_development_international_contract_to_stateside`, RDI→RD) — guard:
  - `ensure!` source kind is `RDI`.
  - This is the §11.3.1 forced transition when the player lands on an NBA roster; allow it
    unconditionally on kind match (no eligibility gate — moving *out* of international is always legal).

### RDI eligibility validator (logic, in `eligibility/`)

`validate_rdi_eligible(contract_model, player_facts, db) -> Result<()>` enforcing §11.3.1:

- Player is RD-eligible (was drafted in the Rookie Draft) — i.e. `EligibilityClassification`
  resolves to `RookieDraftEligible`.
- Player is **currently playing overseas** AND has **never been on an NBA roster / signed an NBA
  contract** ⇒ `ensure!(!has_been_on_nba_roster, …)`. (Source of "playing overseas" + NBA-contract
  signal is ingested data — [spec 12](12-out-of-scope-and-external.md).)
- **Not formerly RD** (§11.3.1 last sentence): a player who was ever a *post-legalization* RD
  contract cannot become RDI. Drafted players are RD/1 initially; the disqualifier is having been
  RD at/after an in-season roster legalization. Determine this from the contract chain
  (`contract_queries` chain lookups) — `bail!` if a prior legalized-RD ancestor exists.
- `is_rdi_eligible` (already on `player`/`league_player`) should be **derived/kept-in-sync** by
  this validator's inputs rather than trusted blindly; treat the existing bool as a cache that
  this validator can correct. Flag mismatch as an open question.

### GraphQL (cross-ref spec 06)

The player/contract GraphQL surface is commented out today (`server/src/graphql/player`,
`contract`). When [spec 06](06-graphql-api-surface.md) wires it:

- Expose `EligibilityClassification`, `has_been_on_nba_roster`, `nba_roster_source/asof` on the
  player types.
- Query: eligible-player lists per context — `veteranAuctionPool`, `rookieDraftEligiblePool`,
  `inSeasonFreeAgentPool` — backed by the logic builders above.
- Mutations (commissioner-only, auth-gated): `setPlayerNbaRosterStatus` (fact correction) and
  `overridePlayerEligibility` (classification override, with reason → audit fields). Keep these
  two distinct per the ingestion section.

## Frontend (Next.js + MUI v7)

### Eligible-player browser per context

- A reusable eligible-player table driven by context = `auction` | `rookieDraft` | `freeAgency`,
  hitting the matching pool query. Columns: name, NBA team / overseas, classification chip
  (Veteran / Rookie-Draft / Ineligible), `nba_roster_asof` freshness, override badge if set.
- **Commissioner eligibility override UI:** an admin-only panel to (a) toggle
  `has_been_on_nba_roster` (fact correction, with source) and (b) set/clear
  `eligibility_override` with a required reason. Show current derived classification vs the
  override so the commissioner sees what they're changing. Surface the audit trail
  (who/when/why). Use MUI v7 tree-shaking imports per project ESLint rule.

## Edge cases & open questions

- **Mid-season NBA signing flips eligibility.** An RDI / rookie-draft-eligible player who signs an
  NBA contract mid-season becomes `VeteranAuctionEligible` and (§11.3.1) must move RDI→RD/1 at the
  next in-season roster legalization (not immediately — §11.3.5). Pool membership for *next*
  auction/draft must reflect the flip; the RDI→RD forced move is handled by the guard above but is
  *deferred to legalization*, so classification and roster-move timing differ. Resolve exact
  trigger timing with [spec 05](05-deadline-scheduler-and-transaction-processor.md) /
  [spec 11](11-roster-legalization-and-in-season.md).
- **basketball-reference data source & freshness.** §3.1.2 names basketball-reference as the
  authority, but the importer reads the NBA player index. Either reconcile sources or treat
  `nba_roster_source` as informational and let commissioner override settle conflicts. Staleness
  window (`nba_roster_asof`) needs a documented SLA — owned by [spec 12](12-out-of-scope-and-external.md).
- **Override audit trail.** Are overrides reversible without losing history? Recommend append-only
  (keep prior override + reason) rather than overwrite. Open.
- **§7.5.3 re-draft eligibility.** Confirm `build_rookie_draft_eligible_pool` ignores all prior
  `contract`/draft history and keys only off classification + current-roster status — verify no
  existing query inadvertently filters out previously-owned players.
- **§7.5.1 sub-category fidelity.** Current data can't distinguish summer-league vs G-League vs
  previously-drafted-foreign vs former-American-collegian-overseas. Decide whether spec 12 must
  ingest these tags or whether `eligibility_override` + a single "draft-eligible" flag is
  acceptable for v1.
- **`is_rdi_eligible` bool vs derived validator** — do we keep the stored bool, derive it, or
  treat it as a correctable cache? Pick one to avoid drift.

## Dependencies

- [spec 01](01-live-auction-engine.md) — consumes `build_veteran_auction_pool` /
  `build_in_season_fa_pool` (FA/UFA/RFA partition + minimum-bid logic).
- [spec 02](02-rookie-draft-engine.md) — consumes `build_rookie_draft_eligible_pool`; owns the
  §7.3.4 same-draft re-draft rule.
- [spec 11](11-roster-legalization-and-in-season.md) — RDI→RD/1 forced transition timing
  (§11.3.1, §11.3.5) and RD-activation legalization.
- Relates: [spec 12](12-out-of-scope-and-external.md) — NBA-roster + overseas + NBA-IR ingestion
  (the upstream data this spec classifies on).
- [spec 06](06-graphql-api-surface.md) — exposes the pools + override mutations.
