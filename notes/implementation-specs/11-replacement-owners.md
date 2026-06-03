# Spec 11 — Replacement Owners

**Rules ref:** §2.4 · **Status:** 🔴 not built · **Priority:** P3 (rare, commissioner-driven)

## Summary

§2.4 covers three commissioner-driven situations where a team changes hands:
1. A departing owner is replaced (§2.4.1) — voluntary quit, ownership transferred ASAP.
2. An owner is forced out (§2.4.2) — collusion/inactivity, same transfer + a reason.
3. After a season, if >1 team is abandoned, an optional replacement-owners draft over the
   combined player pool of those teams (§2.4.4).

This is fundamentally a **`team_user` reassignment + audit** feature, not a roster-mechanics one.
The `team` row and **all** its `contract`/`draft_pick`/`team_update` rows are unchanged — only the
owning `user` changes. The data model already anticipates this: `team_user` carries
`first_end_of_season_year` / `final_end_of_season_year` and a `LeagueRole::Inactive` variant whose
`before_save` validation already requires a `final_end_of_season_year` to be set
(`entity/src/entities/team_user.rs:43-53,129-152`); and `TransactionKind::TeamUpdateConfigChange`
already exists, documented as "ownership/name change"
(`transaction/transaction_entity.rs:143-145`). So most primitives exist — what's missing is the
logic flow that wires them together.

The money side (entry fees not repaid, used by replacement — §2.4.1/§2.4.2) is **out of scope**;
see the Money note and [spec 12](12-out-of-scope-and-external.md).

## Backend

New logic module `logic/src/team_ownership/` already exists (`team_user_access.rs` with
`get_team_user_access_for_user_in_league`). Add an `ownership_transfer.rs` (and, if/when needed,
`replacement_draft.rs`) sibling and re-export from `mod.rs`. Follow the logic conventions: validate
eligibility first, wrap in `db.begin()`…`commit()`, and record a `transaction` + `team_update`
(see `logic/CLAUDE.md` §1–4).

### Ownership transfer (entity/logic: reassign team_user TeamOwner role from old to new user; team + all contracts/picks unchanged; audit record + reason)

New fn, e.g. `logic/src/team_ownership/ownership_transfer.rs::transfer_team_ownership(team_id, new_user_id, end_of_season_year, reason: Option<String>, removal_kind: OwnershipChangeKind, db)`.

Steps (single DB transaction):
1. Resolve the current `TeamOwner` `team_user` for `team_id` (reuse / add to
   `entity/src/queries/team_user_queries.rs` — `find_default_team_user_for_team` and
   `get_team_users_by_team` already exist; add a `get_owner_team_user_for_team` filtering on
   `league_role == TeamOwner` if no exact match exists). `ensure!` exactly one active owner.
2. **Retire the old `team_user`**: set `league_role = LeagueRole::Inactive` and
   `final_end_of_season_year = Some(end_of_season_year)`. This satisfies the existing
   `validate_league_role` invariant (Inactive ⇒ final year set). Do **not** delete the row — it is
   league history (the departing owner must remain attributable on past trades/auctions).
3. **Insert a new `team_user`** for `new_user_id`: `league_role = TeamOwner`,
   `team_id` unchanged, `first_end_of_season_year = end_of_season_year`,
   `final_end_of_season_year = None`, `nickname` = provided or carried over.
4. **Do not touch** any `contract` (chain intact), `draft_pick` (`current_owner_team_id` /
   `original_owner_team_id` unchanged), or `team_update` rows. The team persists; only ownership moves.
5. **Audit**: insert a `transaction` of kind `TransactionKind::TeamUpdateConfigChange` recording
   old `user_id`, new `user_id`, `reason`, and an `OwnershipChangeKind` discriminator
   (`VoluntaryReplacement` | `ForcedRemoval`). The reason string is mandatory for forced removals,
   optional otherwise. Consider an enriched `team_update` payload variant in
   `TeamUpdateData`/`ContractUpdateType` (entity) so the per-team UI can render the change; this is
   the one schema addition this feature needs.

Note `TeamUpdateConfigChange` is currently a no-op arm in transaction processing
(`transaction_entity.rs:312` → `()`); confirm that's acceptable (it carries no contract/asset
mutation to replay) or add a handled arm if [spec 05](05-deadline-scheduler-and-transaction-processor.md)
replay needs to reconstruct ownership history.

### Forced removal (commissioner authz; same mechanism + reason)

Same `transfer_team_ownership` path with `removal_kind = ForcedRemoval` and a **required** `reason`
(collusion / inactivity / disruption — §2.4.2, cross-ref the §12.6 collusion-termination clause).
The only delta vs. voluntary transfer is authorization and the mandatory reason — enforce both at
the GraphQL/authz layer (commissioner-only), not in the entity. The §12.6 case where *all* parties
to a colluded trade are terminated is just N sequential forced-removal calls.

### Replacement-owners draft (optional, commissioner-triggered; combined pool of abandoned teams; players retain salary + contract years; reuse draft-order/selection machinery from spec 02 where possible)

Per §2.4.4 the logistics are "determined if and when the commissioner deems necessary" — so spec a
**flexible, opt-in** mechanism, not a rigid scheduled draft. Strongly recommend treating this as
**YAGNI until a real ≥2-abandoned-teams season occurs** (see Open questions); the section below is a
sketch, not a build order.

When triggered after a `SeasonEnd` deadline:
1. Commissioner flags the set of abandoned `team_id`s (teams whose owner was retired and not yet
   replaced). The combined player pool = every `Active` `contract` on those teams.
2. **Critically, players retain their salary and contract years** (§2.4.4) — this is NOT a
   re-auction and NOT a reset. So the draft transfers existing `contract` rows between teams via the
   existing `contract::trade_to_team`-style helper (a new contract record with previous/original ids
   linked, same `salary` and same contract year/`ContractKind`), **not** `for-auction` / `sign`
   helpers that would reset salary.
3. Reuse [spec 02](02-rookie-draft-engine.md) draft-order + selection machinery where it fits:
   `rookie_draft_selection` table, `RookieDraftSelectionStatus`, the selection-turn ordering. A
   replacement draft is structurally a serpentine/round-based pick over a fixed pool, which spec 02
   already models — parametrize the pool source (abandoned-team contracts) and the "selection
   awards an existing contract, preserving salary/years" branch rather than minting a rookie contract.
   Likely a new `RookieDraftSelectionStatus`-analog or a `kind` flag distinguishing replacement
   selections; avoid forking spec 02's engine if a pool-source parameter suffices.
4. Record each selection as a `transaction` (reuse the selection-transaction pattern) so the
   pool-redistribution is replayable.

### Money note (entry-fee/payout handling is out of scope — commissioner-managed, cross-ref spec 12)

§2.4.1/§2.4.2 say entry fees are not repaid and are used by the replacement, and §2.2.2 says
winnings to a departing owner are paid out only after a replacement is found. **None of this is
modeled in code** — there is no money/ledger entity, and FBKL salaries are fictitious dollars
(§4.1.3). Entry fees, payouts, and the §2.2.2 payout-gating are commissioner-managed out of band.
Do not add money fields here. See [spec 12](12-out-of-scope-and-external.md).

### GraphQL (cross-ref spec 06)

No GraphQL mutation for ownership exists yet (server team/contract resolvers are commented out —
`notes/IMPLEMENTED.md` server table). When [spec 06] wires the commissioner surface, add:
- `transferTeamOwnership(teamId, newUserId, reason)` mutation (commissioner-authz).
- `forceRemoveOwner(teamId, newUserId, reason)` (or fold into the above with a `kind` arg + required
  reason; thin wrapper, business logic stays in `logic/`).
- Optional later: `startReplacementDraft(teamIds)` / selection mutations mirroring spec 02.
- Query exposure of `team_user` history (active owner + retired Inactive rows) so the console can
  show who owned a team when.
Authz: gate on `LeagueRole::LeagueCommissioner` for the acting user; mirror how league mutations
check session/role.

## Frontend (Next.js + MUI v7, commissioner console)

Lives in `webapp-logged-in/`, commissioner-only console (hidden/disabled for `TeamOwner`-role users).

### Transfer-ownership UI, forced-removal with reason, optional replacement-draft setup
- **Transfer ownership**: select team → select replacement user (existing user or invited
  registrant) → optional reason → confirm. Surfaces current owner and warns the action retires them.
- **Forced removal**: same form with a **required** reason field and an extra confirmation step
  (destructive/irreversible framing). Reason persisted to the audit transaction.
- **Replacement-draft setup** (build only if/when needed): multi-select abandoned teams, preview the
  combined pool (players with their *retained* salary + contract year), generate draft order, run
  picks reusing spec 02's draft UI components.
- All forms call backend mutations; no client-side ownership/contract mutation. Reuse MUI form
  patterns from existing league/team admin screens; respect `eslint-plugin-mui-path-imports`.

## Edge cases & open questions

- **In-season vs. offseason transfer.** §2.4.1 explicitly contemplates mid-season transfers (entry
  fees "for the current year will not be repaid and will be used by the replacement owner"). The
  ownership-only model handles both identically (roster persists), so the *mechanism* is
  season-agnostic — the only difference (entry-fee handling) is out of scope. Decide what
  `end_of_season_year` to stamp on the new/old `team_user` for a mid-season transfer (likely the
  current season's end-of-season year, per the §1 end-of-season-year convention).
- **Preserving league history / transaction attribution.** Past `trade_action`, `auction_bid`, and
  `transaction` rows reference the *old* `team_user`/`user`. Retiring (not deleting) the old
  `team_user` keeps those rows valid and attributable. Confirm no query assumes a team has exactly
  one ever-existing `team_user` (e.g. `find_default_team_user_for_team`) — it may need a
  "current owner" filter once multiple `team_user` rows per team exist.
- **Multiple Inactive rows per team.** Repeated turnover ⇒ multiple Inactive `team_user`s per team.
  Ensure owner-resolution queries filter to the single active `TeamOwner`.
- **Replacement draft necessity — YAGNI-until-required.** §2.4.4 itself is conditional ("if… more
  than one team needs a replacement", "at the commissioner's discretion", "logistics determined if
  and when… necessary"). Recommend **not building the replacement-draft engine until a real season
  needs it** — ship only ownership transfer + forced removal now, file the draft as a follow-up
  issue. Flag this explicitly so it isn't gold-plated.
- **Re-activating a returning owner.** Out of scope unless requested; a returning user would just be
  a new `transfer_team_ownership` with them as `new_user_id`.

## Dependencies

- [spec 02](02-rookie-draft-engine.md) — reuse draft-order + selection machinery for the optional
  replacement draft (only if/when built).
- [spec 06] — GraphQL commissioner surface to expose the transfer/removal mutations (none exist yet).
- [spec 05](05-deadline-scheduler-and-transaction-processor.md) — confirm `TeamUpdateConfigChange`
  replay handling if ownership history must be reconstructed.
- [spec 12](12-out-of-scope-and-external.md) — entry-fee/payout/money handling is out of scope.
