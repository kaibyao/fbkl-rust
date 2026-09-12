# Rules-Document Implementation Findings

> **Purpose:** Section-by-section comparison of `notes/2025-08-31-rules_document.md`
> (the FBKL league rules) against what the codebase has actually built (per
> `notes/IMPLEMENTED.md` + source verification on 2026-06-03).
>
> **Status legend:** ✅ built · 🟡 partial / has gaps · 🔴 not built · ⚪ out of scope (external system)
>
> For every gap, a spec lives under `notes/implementation-specs/`. Spec references are linked inline.

---

## How to read this

The codebase today is a **rules engine + import/replay tool**, not a live league platform.
It can *reconstruct* historical seasons from CSV (import-data replays every transaction type)
and it has correct **entity-level contract math** (advancement, discounts, drop penalties).

What it lacks is the **live, forward-looking layer**: an auction engine that accepts bids and
closes on a timer, a rookie-draft engine, the RFA resolution workflow, a deadline scheduler
(`jobs`/`transaction-processor` are stubs), and a GraphQL surface broad enough for a frontend to
drive any of it. Scoring/lineups/playoff results are owned by Fantrax (external) and are correctly
out of scope — but a few derived values (final standings, playoff finish) must still be ingested
to drive draft order.

---

## Section-by-section comparison

### I. Overview (§1–3)

| Rule | Status | Notes |
|------|--------|-------|
| §1.2 Scoring (H2H, 9 cat) | ⚪ | Owned by Fantrax. Not FBKL's job. See [spec 12](implementation-specs/12-out-of-scope-and-external.md). |
| §1.3 Lineups / position eligibility | ⚪ | Fantrax. Position eligibility *data* is imported (`position` table) but lineups are external. |
| §1.4 Playoffs (seeding, bracket, byes) | ⚪/🔴 | Played on Fantrax. But **final standings + playoff finish must be ingested** to drive rookie-draft order & lottery — not built. See [spec 2](implementation-specs/02-rookie-draft-engine.md). |
| §2.1 Communication (Google Group) | ⚪ | External. Email/report generation is a main.rs TODO. |
| §2.2 Buy-in / prize pool | 🔴 | No money/payout tracking entity at all. Low priority (commissioner-managed). |
| §2.4 Replacement owners (+ replacement draft) | 🔴 | No ownership-transfer flow, no replacement draft. See [spec 11](implementation-specs/11-replacement-owners.md). |
| §3 Terminology (contract = years kept; NBA-roster definition) | 🟡 | Contract-as-years modeled correctly. "Has been on active NBA roster" eligibility classification not modeled. See [spec 10](implementation-specs/10-eligibility-and-player-pool.md). |

### II. League Configuration (§4–5)

| Rule | Status | Notes |
|------|--------|-------|
| §4.1 Cap adherence (sum of salaries ≤ cap) | 🟡 | `roster::calculate_team_contract_salary` computes salary; roster-lock validates cap. But **not enforced at bid/trade time** (see specs 1 & 7). |
| §4.2 Cap by period ($200→$210→$230, none post-playoff) | 🟡 | Values exist in constants (`PRE_SEASON_TOTAL_SALARY_LIMIT=200`, `REGULAR_SEASON=210`, `POST_SEASON=230`). **Period→cap selection logic** (which cap applies *now*) is not centralized; the $10 RD-activation bump after auction is not modeled as an event. See [spec 5](implementation-specs/05-deadline-scheduler-and-transaction-processor.md). |
| §4.3 Drop penalties (20%, current-season only) | ✅ | `drop_contract` + `roster` penalty math = `ceil(salary*0.2)`, not carried across seasons. |
| §5.1 In-season limits (22/1 IR/6 RD/1 RDI) | ✅ | `roster_lock::validate_league_rosters` enforces all four. |
| §5.1 Direct-to-IR from offseason 30-man (season-start only) | 🔴 | The "only at season start" special case is not modeled as a guarded transition. See [spec 8](implementation-specs/08-weekly-moves-and-roster-legalization.md). |
| §5.2 Offseason limit (32, no IR) | ✅ | Enforced in preseason roster-lock branch. |

### III. Veteran Auction & Rookie Draft (§6–7)

| Rule | Status | Notes |
|------|--------|-------|
| §6 Sign winning bid → contract | ✅ | `auction::end_veteran_auction` + `sign_auction_contract_to_team`. |
| §6.2 RFA/UFA/FA pool composition | 🟡 | Contract kinds exist; pool *assembly* at keeper deadline not built. See [spec 1](implementation-specs/01-live-auction-engine.md). |
| §6.3 Schedule (RFA-first week, top-150, open nominations, min-bid tiers, daily release) | 🔴 | No auction scheduling at all. See [spec 1](implementation-specs/01-live-auction-engine.md). |
| §6.4 Bid mechanics (cap+roster validation at bid, $1 increment, 24h close, opening ≥ min) | 🔴 | No live bid engine. `auction_bid` table exists but nothing places/validates/closes bids on a timer. See [spec 1](implementation-specs/01-live-auction-engine.md). |
| §7.1–7.3 Draft order, lottery (6/5/4/3/2/1 balls), 2/3-season standings | 🔴 | No draft-order computation, no lottery. See [spec 2](implementation-specs/02-rookie-draft-engine.md). |
| §7.3 Make/pass picks | 🔴 | `rookie_draft_selection` table + import replay exist; no live selection logic/API. See [spec 2](implementation-specs/02-rookie-draft-engine.md). |
| §7.4 Pick salaries ($4/$3/$2/$1/$1), drafted as RD | 🔴 | Round→salary mapping **not in constants**. See [spec 2](implementation-specs/02-rookie-draft-engine.md). |
| §7.3.4 Re-draft ban (dropped-this-draft player can't be re-drafted same draft) | 🔴 | Not modeled. See [spec 2](implementation-specs/02-rookie-draft-engine.md). |
| §7.5 Draft eligibility (never on NBA roster, etc.) | 🔴 | Eligibility classification not modeled. See [spec 10](implementation-specs/10-eligibility-and-player-pool.md). |

### IV. In-Season (§8–13)

| Rule | Status | Notes |
|------|--------|-------|
| §8 In-season FA auctions (opening/all-bid deadlines, 24h, last-hour 30-min extension, min opening = prev salary) | 🔴 | Same missing bid engine as §6, plus the extension + per-week deadline logic. See [spec 1](implementation-specs/01-live-auction-engine.md). |
| §8.1 FA pickup freeze + $20 cap bump | 🔴 | Freeze event / cap transition not modeled. See [spec 5](implementation-specs/05-deadline-scheduler-and-transaction-processor.md). |
| §9.1 Drop penalty 20%, per-player (no combining) | ✅ | Per-contract `ceil(salary*0.2)`. |
| §9.1.4 Nick Adenhart rule (deceased → penalty-free drop) | 🔴 | No override path. See [spec 9](implementation-specs/09-drop-rules-edge-cases.md). |
| §9.1.5 RD/RDI penalty-free drop | ✅ | Penalty applies only to cap-counted contracts. |
| §9.2 Dropped player retains salary as min FA bid; released week after | 🟡 | Pre-drop salary *captured*; min-bid enforcement needs the auction engine. See [spec 1](implementation-specs/01-live-auction-engine.md) + [spec 9](implementation-specs/09-drop-rules-edge-cases.md). |
| §10 IR move/activate | ✅ | `ir::move_contract_to_ir` / `activate_contract_from_ir` with guards. |
| §10 IR accommodation rules (must hit 22-man before IR in-season; drop-from-IR keeps penalty) | 🟡 | Basic moves work; the in-season "accommodate first" sequencing isn't enforced. See [spec 8](implementation-specs/08-weekly-moves-and-roster-legalization.md). |
| §11 RD/RDI contract types, activation, RDI↔RD moves | 🟡 | Built, but `rookie_development_activation` & `..._international` **skip eligibility guards**. See [spec 10](implementation-specs/10-eligibility-and-player-pool.md). |
| §11.8.1 Dropped RD/RDI retains salary as FA min bid | 🔴 **BUG** | `drop_contract.rs:32-33` forces dropped RD/RDI salary to `1`; rule says retain pre-drop salary as in-season FA min bid. See [spec 09](implementation-specs/09-drop-rules-edge-cases.md). |
| §11.9.2 RD/3 → R/2 year-4 conversion (+20% increase) | ✅ | `annual_contract_advancement` handles RD/3 → Rookie year 2. |
| §11.4/§11.5 RD limit overflow at season start (drop/activate beyond 6+1) | 🟡 | Roster-lock validates the 6/1 limits but the simultaneous IR+activate transition isn't a modeled flow. See [spec 8](implementation-specs/08-weekly-moves-and-roster-legalization.md). |
| §12 Trade propose/accept/process, multi-owner, one-way | ✅ | `trade` domain fully handles asset transfer + external invalidation. |
| §12.5.1 Conditional trades (draft-pick position) | 🔴 | `draft_pick_option` modeled, but no code creates an option and no resolution logic exists. See [spec 7](implementation-specs/07-trade-legality-deadline-picks.md). |
| §12 Trade-time cap/roster legality | ✅ | `process_trade` runs T1/T2 per involved team (d1r transaction model, #143) and rejects an illegal result. See [spec 7](implementation-specs/07-trade-legality-deadline-picks.md). |
| §12.3 Trade deadline enforcement | 🔴 | Deadline kind exists; no enforcement gate. See [spec 7](implementation-specs/07-trade-legality-deadline-picks.md). |
| §12.4 Picks tradable 2 years out (window resets after Rookie Draft) | 🔴 | `generate_future_draft_picks` exists but is never called live (fbkl-rust-e1q); the *tradability window* rule isn't enforced. See [spec 7](implementation-specs/07-trade-legality-deadline-picks.md). |
| §13 Weekly moves (intra-week illegal OK, legal by Monday lock; reorderable) | 🔴 | No weekly batching / end-of-week legalization model. See [spec 8](implementation-specs/08-weekly-moves-and-roster-legalization.md). |

### V. Keepers & RFA/UFA (§14–18)

| Rule | Status | Notes |
|------|--------|-------|
| §14.1 Keeper limits (14 players / $100, excl. RD/RDI) | ✅ | `keeper_deadline` enforces both via constants. |
| §14.2 Salary increase 20% rounded up | ✅ | Advancement applies the increase. |
| §14.3 Contract years / max length | ✅ | Year tracking + max-length transitions in `annual_contract_advancement`. |
| §14.4 Keeper deadline = RFA/UFA announcement | 🟡 | Keeper deadline processing exists; the **RFA/UFA designation + original-owner snapshot** at that moment isn't a modeled event. See [spec 3](implementation-specs/03-rfa-resolution-and-compensation.md). |
| §15.1–15.2 RFA discount (10% off final bid) | 🟡 | Discount math ✅ (`sign_rfa_or_ufa_contract_to_team`); but **no resolution workflow**. |
| §15.2 RFA draft-pick compensation (bid→round tiers, "or better", eligible-pick rules) | 🔴 | Entirely unbuilt. See [spec 3](implementation-specs/03-rfa-resolution-and-compensation.md). |
| §15.3 RFA process (48h raise → 48h match → forfeit/sign) | 🔴 | No timed workflow. See [spec 3](implementation-specs/03-rfa-resolution-and-compensation.md). |
| §15.4 RFA rights retained on trade, lost on drop+repickup | 🟡 | Contract chains preserve rights through trade; drop→new-V resets correctly. Needs the designation event to surface it. See [spec 3](implementation-specs/03-rfa-resolution-and-compensation.md). |
| §16 UFA discounts (5yr 20%/max $8; 3yr 10%/max $5; min $1) | 🔴 **BUG** | Discount math exists but **omits the $8/$5 max caps** (`free_agent_extension.rs:80-90` only does ceil×rate, floor 1). See [spec 4](implementation-specs/04-ufa-rfa-discount-caps.md). |
| §16.4 Veteran exception transfers on trade, lost on drop+repickup | 🟡 | Same as §15.4. |
| §17/§18 Reference tables | ✅ (doc) | Encoded as the advancement/discount logic above. |

---

## Gap summary → spec index

| # | Spec | Covers | Priority |
|---|------|--------|----------|
| 01 | [Live auction engine](implementation-specs/01-live-auction-engine.md) | §6, §8 | P0 |
| 02 | [Rookie draft engine](implementation-specs/02-rookie-draft-engine.md) | §7, §1.4 (standings) | P0 |
| 03 | [RFA resolution & compensation](implementation-specs/03-rfa-resolution-and-compensation.md) | §14.4, §15 | P1 |
| 04 | [UFA/RFA discount caps (bugfix)](implementation-specs/04-ufa-rfa-discount-caps.md) | §16 | P1 |
| 05 | [Deadline scheduler & transaction-processor](implementation-specs/05-deadline-scheduler-and-transaction-processor.md) | §4.2, §8.1, infra | P0 |
| 06 | [GraphQL API surface](implementation-specs/06-graphql-api-surface.md) | server gap | P0 |
| 07 | [Trade legality, deadline & picks](implementation-specs/07-trade-legality-deadline-picks.md) | §12 | P1 |
| 08 | [Weekly moves & roster legalization](implementation-specs/08-weekly-moves-and-roster-legalization.md) | §5, §10, §11.4, §13 | P1 |
| 09 | [Drop rules edge cases](implementation-specs/09-drop-rules-edge-cases.md) | §9.1.4, §9.2 | P2 |
| 10 | [Eligibility guards & player pool](implementation-specs/10-eligibility-and-player-pool.md) | §3, §7.5, §11 | P1 |
| 11 | [Replacement owners](implementation-specs/11-replacement-owners.md) | §2.4 | P3 |
| 12 | [Out-of-scope & external systems](implementation-specs/12-out-of-scope-and-external.md) | §1–2 (Fantrax) | — |

**Headline:** the contract/transaction *math* is solid. The missing system is the **live operational
layer** — auctions, draft, RFA workflow, deadline automation, and the API to expose them. Specs 01,
02, 05, 06 are the critical path; everything else layers on top.
