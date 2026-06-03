# Spec 01 — Live Auction Engine
**Rules ref:** §6, §8 · **Status:** 🔴 not built · **Priority:** P0

## Summary

The contract-signing tail of auctions exists (`end_veteran_auction`, `end_fa_auction`,
`sign_auction_contract_to_team`), but the entire *live* engine in front of it is missing: placing
and validating bids against cap+roster, the §6.4.1 winning-bid accounting, auction lifecycle/state,
timed close (24h-since-last-bid + the §8.3.2 last-hour rolling extension), and the season schedule
that releases players and slides min-bid tiers. This spec covers both auction modes — preseason
Veteran Auction (§6) and In-Season FA (§8) — plus the shared bid mechanics they have in common.

## Backend

### entity/ (new columns/tables, enums)

Current state: `auction` has `id, kind, minimum_bid_amount, start_timestamp, soft_end_timestamp,
fixed_end_timestamp, contract_id`. `auction_bid` has `id, bid_amount, comment, auction_id,
team_user_id`. `AuctionKind` = { `InSeasonFreeAgent` ("FreeAgent"), `PreseasonVeteranAuction` }
(a commented-out `PreseasonFreeAgent` variant exists — uncomment for open-nomination/Week-1).
`soft_end_timestamp` already = start+24h, `fixed_end_timestamp` already = start+48h, but nothing
*recomputes* `soft_end_timestamp` on each bid (it must — see logic) and nothing acts on either.

New/changed:

1. **`auction.status`** — new column + enum `AuctionStatus` { `Pending` (scheduled, not yet
   open for bids), `Open`, `Closed` (timer elapsed, awaiting `end_*_auction`), `Completed`
   (contract signed), `Expired` (closed with no bids) }. Replaces today's implicit
   "open if now < soft_end" inference. `end_veteran_auction`/`end_fa_auction` set
   `Completed`/`Expired`.
2. **`auction.soft_end_timestamp` semantics** — keep the column, but it becomes mutable: each
   accepted bid sets `soft_end = bid_time + 24h` (§6.4.4 / §8.3.1). Add query
   `extend_auction_soft_end(auction_id, new_soft_end, db)`.
3. **`auction.fixed_end_timestamp` semantics** — for FA auctions this is the *all-bid deadline*
   (Sun 8pm CT) and is mutable by the §8.3.2 rolling extension; for veteran auctions it is unused
   (veteran close is purely 24h-since-last-bid). Add query `extend_auction_fixed_end(...)`.
4. **`auction.minimum_bid_amount`** — already present; the tier slide-down (§6.3.4) mutates it.
   Add `update_auction_minimum_bid(auction_id, new_min, db)`.
5. **`auction_schedule`** (new table) — drives §6.3 veteran-auction release + tiers. Columns:
   `id, league_id, end_of_season_year, player_id, scheduled_release_date (NaiveDate),
   nomination_rank (i16, NULL for open-noms), min_bid_tier (i16), is_rfa_week (bool)`. One row
   per pooled player. `min_bid_tier` indexes into:
6. **`min_bid_tier_config`** (new table or JSON column on `league` season config) — ordered tiers
   `(tier_index i16, min_bid_amount i16)`. Set per-season (§6.3.6). The slide rule (§6.3.4-.5)
   only ever moves a player's auction `min_bid_amount` down to the *next* tier's value.
7. **`auction.is_rfa` / link to RFA flow** — not new here; RFA-specific resolution (48h
   raise/match, pick compensation) is **spec 03**. This engine only needs to know an auction is
   RFA-restricted so it can reject the original owner's bids and route close → spec 03 instead of
   straight signing. Add `auction.original_owner_team_id (Option<i64>)` (NULL except RFA/UFA).
8. **`auction_bid` invalid-bid record** — to honor §6.4.1 "null and void" auditably, add
   `auction_bid.is_valid (bool, default true)` and `auction_bid.invalid_reason (Option<String>)`,
   OR (preferred) reject invalid bids before insert and never persist them. Pick rejection;
   keep `auction_bid` rows = valid winning-chain only.

Migrations: one new `m_*_create_auction_schedule`, one `m_*_alter_auction_add_status_and_owner`,
one `m_*_create_min_bid_tier_config` (or fold tiers into existing season config migration).

### logic/ (new functions)

Follow conventions: validate before mutate; every *winning state change* (signing) = transaction +
team_update (already done by `sign_auction_contract_to_team`); delegate persistence to
`entity/src/queries`; wrap multi-step in `db.begin()…commit()`. Note: a *bid* is not a roster state
change, so it produces **no** transaction/team_update — only auction-win signing does.

New module `logic/src/auction/place_bid.rs`:

1. **`place_auction_bid(auction_id, bidding_team_user_id, bid_amount, comment, now, db) ->
   Result<auction_bid::Model>`** — the core entry point. Steps, all inside a db txn:
   - Load auction; `ensure!(status == Open)`. `ensure!(now < soft_end && now < fixed_end)`
     (for FA the all-bid/opening-bid deadline gate; see scheduler section).
   - **RFA guard:** `ensure!(auction.original_owner_team_id != Some(bidder_team_id))` (§6.2.2.3 /
     §15.3.1).
   - **Increment + opening rules (§6.4.2-.3, §8.3.3-.4):** if no prior bid,
     `ensure!(bid_amount >= auction.minimum_bid_amount)`; else
     `ensure!(bid_amount >= latest_bid.bid_amount + 1)`. (Today `insert_auction_bid` only enforces
     `> previous`; tighten to `>= prev + $1` and reuse it.)
   - **§6.4.1 validity check** — `validate_bid_cap_and_roster(...)` below. On failure → `bail!`
     (the previous bid stays winning; "null and void"). This is the subtle part.
   - On success: `auction_queries::insert_auction_bid(...)`, then
     `extend_auction_soft_end(auction_id, now + 24h)`, then apply §6.3.4 tier-slide is **not** here
     (that fires on *non*-bid at a daily tick, see scheduler), then for FA apply the §8.3.2
     last-hour extension (below).

2. **`validate_bid_cap_and_roster(bidding_team_id, this_bid_amount, deadline/cap_period, now, db)
   -> Result<()>`** — the §6.4.1 check. Counts the bidder's commitments *including their own
   currently-winning bids*:
   - Gather all `Open` auctions in this league/season where the bidder is the current top bidder
     (`get_latest_bid().team == bidder` and `is_valid`). Call these `winning_bids`.
   - **Cap (veteran only, §6.4.1.1):** `committed = team_current_salary +
     sum(winning_bids.bid_amount) + this_bid_amount` (if this auction is already in `winning_bids`,
     swap its old amount for `this_bid_amount`). `ensure!(committed <= team_max_cap)`. Reuse
     `roster::calculate_team_contract_salary_with_model` for `team_current_salary` and the
     period cap ($200 during veteran auction per §4.2.1).
   - **Open roster space (§6.4.1.2):** `roster_used = active_contract_count +
     count(winning_bids) (+1 if this auction not already counted)`.
     `ensure!(roster_used <= roster_limit)` (preseason limit 32 = `PRESEASON_*` constant during
     veteran auction).
   - **§8 difference:** In-Season FA explicitly does **not** cap-gate bids — §8.3.5 lets owners bid
     above free cap as long as they accommodate via drops/trades on win. So
     `validate_bid_cap_and_roster` must be **skipped (or cap-only-warn)** when
     `auction.kind == InSeasonFreeAgent`. Gate this on kind; do not apply §6.4.1 to §8.

3. **Veteran pool assembly** — new `logic/src/auction/assemble_veteran_pool.rs`:
   **`assemble_veteran_auction_pool(league_id, end_of_season_year, db)`** — run at/after keeper
   deadline. For every NBA-veteran not kept, produce a pooled `contract` of kind FreeAgent / UFA /
   RFA (reuse `get_or_create_player_contract_for_veteran_auction`, which already validates
   `VALID_VETERAN_AUCTION_FA_TYPES`), then build `auction_schedule` rows: RFAs flagged
   `is_rfa_week`, top-150 ranked players get `nomination_rank` + staggered `scheduled_release_date`,
   the rest are open-nomination (rank NULL). Tier assignment writes `min_bid_tier`. UFA/RFA min bid
   = their carry salary (§15.3.1 RFA = 4th-year salary). The ranking source (ESPN/Yahoo) is an
   import input — accept a ranked `Vec<player_id>` argument; do not scrape here.

4. **Open a scheduled auction** — `open_scheduled_auction(auction_schedule_row, now, db)`:
   creates the `auction` (`start_new_auction_for_nba_player` already exists, generic over
   `AuctionKind`) with `status=Open`, `minimum_bid_amount` from the tier, and
   `original_owner_team_id` for RFA/UFA. Fired by the daily release tick (scheduler).

5. **Tier slide-down (§6.3.4-.5)** — `slide_unbid_auctions_down_a_tier(league_id, season, now,
   db)`: for each `Open` veteran auction with **zero bids** at the daily tick, set
   `minimum_bid_amount = next_lower_tier.min_bid_amount`. Per §6.3.5, sliding a player into a tier
   does **not** push the existing last player of that tier further down — so the slide is a pure
   per-auction lookup of the next configured tier value, never a cascade.

6. **FA opening-bid min (§8.3.3)** — when opening a *new* in-season FA auction (the
   `PreseasonFreeAgent`/in-season nominate path): min opening bid = $1, **unless** the player was
   previously owned in the current season → min = that previous in-season salary (applies to RD/RDI
   too, §8.3.3). Need `contract_queries` lookup of the player's most-recent dropped/owned salary
   this season (the dropped-contract carry-salary helpers noted in IMPLEMENTED already exist —
   reuse them). Set `auction.minimum_bid_amount` accordingly.

7. **Cleanup of existing gotchas (do these here):**
   - `end_fa_auction` treats no-bid as a **hard error** — change to mirror the veteran path:
     no bid ⇒ `expire_contract` ⇒ player back to FA pool, set `AuctionStatus::Expired`.
   - `end_fa_auction`'s `maybe_override_effective_date` param is dead/unused — either wire it
     (stamp the team_update effective date like the veteran path does via
     `update_team_update_for_preseason_veteran_auction`) or drop the param. Prefer wiring it for
     symmetry.
   - Tighten `insert_auction_bid`'s validation from `> previous` to `>= previous + 1` (the $1
     increment is a hard rule, §6.4.2/§8.3.4) — or move all validation into `place_auction_bid`
     and reduce `insert_auction_bid` to a pure insert.

### scheduler/jobs (cross-ref spec 05)

The `jobs` and `transaction-processor` crates are stubs; the timer lives in **spec
05 (`05-deadline-scheduler-and-transaction-processor.md`)** — do not redesign it here. This engine
just supplies the functions spec 05 must invoke:

- **Per-minute close tick:** find `Open` auctions where `now >= soft_end_timestamp` (24h since last
  bid) AND (for FA) `now >= fixed_end_timestamp` (all-bid deadline + any rolling extension) →
  call `end_veteran_auction` / `end_fa_auction`. RFA closes route to spec 03 instead of signing.
- **Daily release tick (veteran, §6.3.3):** `open_scheduled_auction` for rows whose
  `scheduled_release_date <= today`, then `slide_unbid_auctions_down_a_tier` for unbid open
  auctions.
- **Weekly FA deadlines (§8.2):** opening-bid deadline Fri 11:59pm CT (after this, no *new* FA
  auctions may be opened — gate the nominate path), all-bid deadline Sun 8pm CT
  (`auction.fixed_end_timestamp`). These map to `DeadlineKind::FreeAgentAuctionEnd` /
  `Week1FreeAgentAuctionStart/End` which already exist.
- **§8.3.2 rolling extension** is applied *inside* `place_auction_bid` (not the scheduler): if
  `now` is within 1h of `fixed_end_timestamp`, set `fixed_end = fixed_end + 30min`; subsequent bids
  within the 30-min window each push +30min until a 30-min gap. (Worked example §8.5: $5 bid 7:15pm
  → 8:30pm; $6 at 7:42pm → still 8:30; $7 at 8:13pm → 9:00pm; quiet → Joe wins $7.) Unit-test this
  example exactly.

### GraphQL (cross-ref spec 06)

Resolvers don't exist yet (server team/player/contract domains are commented out — see IMPLEMENTED).
Spec 06 owns wiring; this engine needs:
- **Query** `auction(id)`, `openAuctions(leagueId, seasonYear)`, `myWinningBids` (drives the
  §6.4.1 committed-cap display).
- **Mutation** `placeAuctionBid(auctionId, bidAmount, comment)` → `place_auction_bid`; must surface
  the null-and-void rejection as a typed error (insufficient cap / no roster space / below min /
  RFA-original-owner / auction closed).
- **Subscription** `auctionUpdated(auctionId)` (or polling fallback) for live bid + countdown.

## Frontend (webapp-logged-in, Next.js + MUI v7 + urql + react-hook-form)

- **Auction list page** (`/auctions`): MUI `DataGrid` of open auctions — player, current bid,
  current winner, min bid, time-to-close. Veteran mode groups by `scheduled_release_date`; FA mode
  shows opening-bid vs all-bid deadline columns.
- **Auction detail / bid panel:** bid history (from `auction.get_bids`), a react-hook-form bid
  input (validate `>= min` / `>= current + 1` client-side; server is source of truth), comment
  field. Disable bid if RFA-original-owner.
- **Live bid display:** urql subscription (or 5s poll) on `auctionUpdated`; optimistic bid then
  reconcile; toast the null-and-void rejection reason.
- **Deadline countdowns:** two timers per FA auction — the rolling soft-end (24h-since-last-bid) and
  the all-bid deadline (with visible +30min bumps when the §8.3.2 extension fires). Veteran auctions
  show only the 24h soft-end. Render all times in CT (league tz) regardless of viewer locale.
- **Committed-cap meter:** show `team_current_salary + sum(my winning bids)` vs cap so the bidder
  sees the §6.4.1 headroom before a bid is rejected.

## Edge cases & open questions

- **§6.4.1 self-counting:** when re-bidding on an auction you already lead, the new amount
  *replaces* the old in the committed total — easy off-by-one. Cover with a test.
- **Tier slide vs a fresh open:** a player opened today at tier N should not also slide same-day;
  slide only auctions that have been open ≥1 daily tick with no bids. Define the exact tick offset
  in spec 05.
- **No-bid veteran auction** already expires correctly; ensure the FA fix matches.
- **RFA hand-off:** close of an RFA auction must NOT sign immediately — it enters the 48h
  raise/match flow (spec 03). This engine sets `Closed`; spec 03 transitions to `Completed`.
- **Concurrency:** two bids racing on the same auction — wrap read-latest-bid + insert in the txn
  and rely on `>= prev + 1` to reject the loser; consider a row lock / unique-ish guard. Decide.
- **Open question:** does the opening-bid deadline (Fri) freeze only *new* auctions while existing
  ones keep taking bids until Sun? Per §8.2.1 yes — confirm and gate `open_*_auction` on Fri,
  `place_auction_bid` on Sun (+extensions).
- **Open question:** where does the per-season ranked top-150 list + tier config get entered —
  import-data CLI, commissioner GraphQL mutation, or seed? Likely import-data; confirm in spec 09/10.

## Dependencies

- **[spec 04](04-ufa-rfa-discount-caps.md)** — discount caps; should land first so signed UFA/RFA
  salaries are correct.
- **[spec 05](05-deadline-scheduler-and-transaction-processor.md)** — owns the timer that fires
  close/release/deadline ticks. This spec defines the functions; 05 schedules them.
- **[spec 06](06-graphql-api-surface.md)** — auction queries/mutations/subscription wiring.
- **spec 03 (RFA resolution & compensation)** — consumes `Closed` RFA auctions for the 48h
  raise/match + draft-pick forfeit.
- **specs 09/10** — per-season pool/ranking/tier import + commissioner config entry.
