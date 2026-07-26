# Spec 10 — Eligibility & Player Pool
**Rules ref:** §3, §6.2, §7.5, §8.4, §11 · **Status:** 🔴 classification + guards missing · **Priority:** P1

## Summary

The whole league splits players into two acquisition pools by one fact: **has the player ever
appeared in an in-season NBA game?** (§3.1.2 — a regular season or playoff appearance per
basketball-reference.com; roster presence alone, summer league, preseason, and G League do not
count). That single pivot decides:

- **Veteran Auction pool** (§6.2.1): all players who *have* played in an NBA game.
- **Rookie Draft pool** (§7.5): a constrained set of players who have *never* played in an NBA game.
- **In-season FA pool** (§8.4): the union of the two above; everyone ineligible for both auction
  and draft stays ineligible for FA.

A **second, broader** fact — has the player ever been on an active NBA roster (§3.1.3, the NBA
entry on basketball-reference.com regardless of appearances) — is deliberately *not* the pool
pivot. It is used only for RDI eligibility, because §11.3.1 forces RDI→RD/1 as soon as a player is
"on an NBA roster / signed to an NBA contract". A rostered player who never appeared in a game is
therefore still rookie-draft-eligible but already RDI-ineligible, so both facts must be stored.

Both are **point-in-time, not career booleans.** Eligibility is a question about a season: the same
player is rookie-draft-eligible the season before his debut and a veteran after it, the pool
builders already take an `end_of_season_year`, and historical replay asks about past seasons. So the
stored facts are a first-NBA-season stamp plus a career "ever appeared" bool, and every caller says
which season it means — pool builders their pool's, contract guards the contract's.

The season comparison is **strictly earlier**, not same-or-earlier. The stamp is season-granular
while the rules turn on *where in* a season the player arrived: §11.3.5 gives an RDI who lands on an
NBA roster mid-season until the *next* legalization to move, and both a mid-season signing and a
draft-and-stash record the season they arrived in. Same-or-earlier would reject exactly the
legalization §11.3.5 protects — it rejects 2 of the 25 historical RD→RDI moves (Tristan Vukcevic,
who signed with Washington in March 2024, and Saliou Niang, stashed under his draft season).

Neither fact is **modeled today.** `player`/`league_player` carry only `is_rdi_eligible: bool`
and (on `player`) `PlayerStatus { Active, Retired }` — neither encodes NBA history nor a
draft-vs-auction classification. Spec covers: (1) an eligibility model on the player entities,
(2) where the NBA facts are ingested from (cross-ref [spec 12](12-out-of-scope-and-external.md)),
(3) pool-assembly functions in `logic/`, (4) the **missing eligibility guards** on
`rookie_development_activation` + `rookie_development_international` (a known gap — they skip the
pre-mutation `ensure!`/`bail!` that `ir`/`drop` enforce), (5) an RDI eligibility validator, and
(6) a commissioner override for the §3.1.2 / §11.3.6 "decided by the commissioner" edge cases.

NBA game/roster status and NBA-IR status are derived/ingested data — see
[spec 12](12-out-of-scope-and-external.md) for the source and freshness contract.

## Backend

### Player eligibility model (entity)

Add to **`player`** (the real-world NBA player; `entity/src/entities/player.rs`) and mirror on
**`league_player`** (`entity/src/entities/league_player.rs`, used for drafted-but-not-yet-NBA
custom players) the following fields, via a new migration:

- `has_played_nba_game: bool` — whether the player ever appeared in an in-season NBA game, over his
  whole career. On `league_player`, `false` is the normal case (a custom player exists *because* he
  has no NBA entry yet).
- `nba_first_season_end_of_season_year: Option<i16>` — the season the player first appeared in NBA
  data, `None` for never. Setting it is the §11.3.1 trigger to move RDI→RD/1. Required whenever
  `has_played_nba_game` is true (you cannot appear in a game with no season to appear in), so that
  pair is rejected at the write boundary.
- `nba_roster_source: NbaRosterSource` enum `{ BasketballReference, Espn, Nba, CommissionerOverride, Unknown }`
  — provenance of both facts, which one ingest sets together.
- `nba_roster_asof: Option<DateTimeWithTimeZone>` — when the facts were last evaluated (freshness;
  they are only as good as the last ingest — see edge cases). `None` means never ingested, which is
  how a dump-loaded database is distinguishable from one an importer has filled in.

The two are combined into as-of-a-season predicates in `logic`, never read raw by rule code:

- `was_on_nba_roster_before(season)` — §3.1.3: `nba_first_season < season`.
- `had_played_nba_game_before(season)` — §3.1.2: the above, narrowed by `has_played_nba_game`.
- `eligibility_override: Option<EligibilityClassification>` — commissioner manual override
  (§3.1.2, §11.3.6). When `Some`, it wins over the derived classification.
- `eligibility_override_reason: Option<String>` + `eligibility_override_by_team_user_id` /
  `..._at` — audit trail for the override.

New enum (entity, `async_graphql::Enum` + `DeriveActiveEnum`, same pattern as `PlayerStatus`):

```rust
pub enum EligibilityClassification {
    RookieDraftEligible,    // never played an NBA game AND in the §7.5.1 eligible set
    VeteranAuctionEligible, // has played in an NBA game (§6.2.1)
    Ineligible,             // §7.5.2 / §8.4.2 — current college/HS, undrafted foreign non-collegian, etc.
}
```

Classification is **derived**, not stored as a column, by a pure fn (see pool assembly). The
stored `eligibility_override` short-circuits it. Reasoning for keeping it derived: `RookieDraft`
vs `Veteran` membership shifts over time (a draft-eligible player who signs an NBA contract
mid-cycle flips to veteran once he debuts) and must always reflect current `has_played_nba_game`.

`PlayerStatus { Active, Retired }` stays as-is — it answers "show in search?", a different
question than "which pool?". Do not overload it.

### Ingestion (where NBA-roster status comes from)

`import-data/src/real_world/import_players.rs:312` currently sets `PlayerStatus` from
`player.to_year == 2024` only — it derives nothing about NBA-roster history. Extend the importer
(NBA player index `data/nba_player_index_*.json`; cross-ref [spec 12](12-out-of-scope-and-external.md)):

- A player present in the NBA player index ⇒ `nba_first_season_end_of_season_year = FROM_YEAR + 1`
  (index presence *is* the §3.1.3 NBA entry, and `FROM_YEAR` is a season *start* year so its
  end-of-season year is one later), `nba_roster_source = Nba` (or `BasketballReference` once that
  source is wired), `nba_roster_asof = now()`.
- `has_played_nba_game` comes from the index's career `PTS` column: it is null only for players with
  no in-season appearance (86 of 5087 rows in `nba_player_index_2025-07-01.json` — e.g. Terrico
  White, Da'Sean Butler, Magnum Rolle, plus current-year draftees who have not debuted). `PTS = 0.0`
  means he played and scored none, so null-vs-zero is the whole signal. The index carries no `GP`
  column; if spec 12 wires a source that does, prefer `GP > 0`.
- A `league_player` (drafted, no NBA entry) ⇒ `has_played_nba_game = false` and a `None` first
  season, source `Unknown` until an ingest confirms, asof set on each run.
- **Manual / commissioner path:** a mutation to set both facts +
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

- `classify_player(player_facts, end_of_season_year) -> EligibilityClassification` — pure. Order:
  1. If `eligibility_override.is_some()` ⇒ return it.
  2. If `had_played_nba_game_before(end_of_season_year)` ⇒ `VeteranAuctionEligible` (§6.2.1). The
     §3.1.3 roster fact is *not* consulted here — a rostered player who never appeared stays in the
     draft pool.
  3. Else if in the §7.5.1 rookie-eligible set (drafted-this-year, declared-undrafted,
     summer-league-never-played, G-League-never-played, previously-drafted-foreign-never-played,
     former-American-collegian-overseas-never-played) ⇒ `RookieDraftEligible`.
  4. Else (§7.5.2: current college/HS, undrafted foreign non-collegian, other) ⇒ `Ineligible`.
  - Note: the §7.5.1 sub-categories are not yet representable from current data (we only have
    "in NBA index" vs "custom league_player"). Until [spec 12](12-out-of-scope-and-external.md)
    or [spec 02](02-rookie-draft-engine.md) lands richer source tags, approximate
    `RookieDraftEligible = !has_played_nba_game && (is a drafted league_player || flagged
    draft-eligible)` and rely on `eligibility_override` for edge cases. Capture the data gap as
    an open question below.
- `build_veteran_auction_pool(league_id, end_of_season_year, db)` — players classified
  `VeteranAuctionEligible` that are not currently rostered keepers, partitioned into FA / UFA /
  RFA per §6.2.2 by reading keeper outcomes (cross-ref [spec 01](01-live-auction-engine.md)
  auction pool + the keeper-deadline results in `deadline_processing/keeper_deadline`).
- `build_rookie_draft_eligible_pool(league_id, end_of_season_year, db)` — players classified
  `RookieDraftEligible`. **§7.5.3:** prior league draft/ownership does **not** affect eligibility —
  a previously-drafted-but-now-unrostered player who never played is still eligible. So the filter keys
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
  contract** ⇒ `ensure!(!facts.was_on_nba_roster_before(contract.end_of_season_year), …)`. This is
  the §3.1.3 fact, deliberately broader than the §3.1.2 pool pivot, so the check does not collapse
  into the `RookieDraftEligible` test above. (Source of "playing overseas" + NBA-contract signal is
  ingested data — [spec 12](12-out-of-scope-and-external.md).)
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

- Expose `EligibilityClassification`, `has_played_nba_game`,
  `nba_first_season_end_of_season_year`, `nba_roster_source/asof` on the player types, plus the
  season the classification was derived for so a cached client can't mistake it for timeless.
- Query: eligible-player lists per context — `veteranAuctionPool`, `rookieDraftEligiblePool`,
  `inSeasonFreeAgentPool` — backed by the logic builders above.
- Mutations (commissioner-only, auth-gated): `setPlayerNbaStatus` (both fact corrections) and
  `overridePlayerEligibility` (classification override, with reason → audit fields). Keep these
  two distinct per the ingestion section.

## Frontend (Next.js + MUI v7)

### Eligible-player browser per context

- A reusable eligible-player table driven by context = `auction` | `rookieDraft` | `freeAgency`,
  hitting the matching pool query. Columns: name, NBA team / overseas, classification chip
  (Veteran / Rookie-Draft / Ineligible), `nba_roster_asof` freshness, override badge if set.
- **Commissioner eligibility override UI:** an admin-only panel to (a) toggle
  `has_played_nba_game` / `nba_first_season_end_of_season_year` (fact correction, with source) and (b) set/clear
  `eligibility_override` with a required reason. Show current derived classification vs the
  override so the commissioner sees what they're changing. Surface the audit trail
  (who/when/why). Use MUI v7 tree-shaking imports per project ESLint rule.

## Edge cases & open questions

- **Mid-season NBA signing and debut are different facts, and the stamp is only season-granular.**
  Being rostered sets the first-NBA-season stamp, which (§11.3.1) forces RDI→RD/1 at the next
  in-season roster legalization, not immediately (§11.3.5) — hence the strictly-earlier comparison.
  Only the *debut* sets `has_played_nba_game`, which moves him to `VeteranAuctionEligible`. The two
  can be a season apart: a stashed player signed in January who never appears is RDI-ineligible from
  the following season on and still rookie-draft-eligible. Within a season the stamp cannot tell a
  July signing from a March one, so a player signed *before* the season starts is treated as having
  the §11.3.5 grace period he isn't strictly owed — deliberately permissive, since the guard should
  not reject history that was legal. Resolve exact trigger timing with
  [spec 05](05-deadline-scheduler-and-transaction-processor.md) /
  [spec 11](11-roster-legalization-and-in-season.md).
- **basketball-reference data source & freshness.** §3.1.2 names basketball-reference as the
  authority, but the importer reads the NBA player index and infers "played" from a non-null career
  `PTS`. Either reconcile sources or treat `nba_roster_source` as informational and let commissioner
  override settle conflicts. A source exposing `GP` would be strictly better than the PTS proxy.
  Staleness window (`nba_roster_asof`) needs a documented SLA — owned by
  [spec 12](12-out-of-scope-and-external.md).
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
