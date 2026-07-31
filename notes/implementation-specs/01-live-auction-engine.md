# Spec 01 — Live Auction Engine
**Rules ref:** §6, §8 · **Status:** 🟡 built, being retimed · **Priority:** P0

## Summary

The contract-signing tail of auctions exists (`end_veteran_auction`, `end_fa_auction`,
`sign_auction_contract_to_team`), but the entire *live* engine in front of it is missing: placing
and validating bids against cap+roster, the §6.4.1 winning-bid accounting, auction lifecycle/state,
timed close (24h-since-last-bid, shortened to 1h in the crunch window), and the season schedule
that releases players and slides min-bid tiers. This spec covers three auction modes — preseason
Veteran Auction (§6), preseason FA Auction (open nomination + Week 1), and In-Season FA (§8) — plus
the shared bid mechanics they have in common.

## Timing rules — authoritative

`notes/2025-08-31-rules_document.md` is a dated snapshot of the league's voted rules. It fully
covers the in-season §8 timings; it is *silent* on the two preseason auctions' hard deadlines, which
this section supplies. Every auction, every mode, closes by one rule:

```
no bids yet   → veteran: the tier slide is the only clock (see below); FA: unreachable, the nomination is a bid
has bids      → close_at = min(last_bid + quiet_window, all_bid_deadline)   clamped to the hard deadline

quiet_window     = 24h (§6.4.4 / §8.3.1); preseason only: 1h inside the crunch window
all_bid_deadline = in-season FA: Sunday 8:00pm CT, rolled forward per §8.3.2 (below)
                   veteran + preseason FA: none — those modes use the crunch window instead
hard deadline    = PreseasonFinalRosterLock  (veteran + preseason FA)
                   next InSeasonRosterLock   (in-season FA)
crunch window    = preseason only. Starts at (hard deadline − 24h), moved forward to 8:00am CT if
                   that lands between 00:00 and 08:00 CT — it never opens while owners are asleep.
```

The hard deadline is **absolute**: an auction still taking bids is force-closed there and the last
bidder wins. That is what bounds the §8.3.2 extension chain, which the rules doc leaves open-ended.

### §8.3.2 rolling extension (in-season only)

A late bid pushes the all-bid deadline out 30 minutes. The trigger has **two widths**, and getting
this wrong is the easy bug:

- deadline not yet extended → a bid within the final **60 minutes** rolls it +30min (§8.3.2's
  "Sunday 7:00 PM-8:00 PM CT")
- deadline already extended → a bid within the final **30 minutes** rolls it +30min again, until 30
  quiet minutes pass

Against §8.5's worked example with the original 8:00pm deadline: 7:15pm bid → 8:30pm; 7:42pm bid →
still 8:30pm (48 min out, and the trigger is now 30 min); 8:13pm bid → 9:00pm; quiet → the 8:13pm
bidder wins. A single flat 30-minute trigger does **not** reproduce this — it would let the 7:15pm
bid close the auction at 8:00pm. Commit `59e607c` shipped exactly that flat version and its test
masked it by starting the deadline at 8:30pm instead of 8:00pm; commit `5c67a1c` then removed the
extension entirely.

### Consequences worth stating plainly

- A bid always buys its full quiet window, bounded only by `all_bid_deadline` and the hard deadline.
  No *immovable* per-auction end time may cut off live bidding — that was the old
  `fixed_end_timestamp` model and it was wrong.
- An unbid **veteran** auction has no expiry date. It slides one min-bid tier per day (§6.3.4); the
  day the slide finds no lower tier, the auction expires and the player becomes a $1 FA (§6.1.2).
  The tier ladder *is* the clock.
- An **FA** auction (preseason or in-season) is opened *by* a bid, so it always has one.
- In-season: nominations are frozen at Friday 11:59pm CT, but **bids on already-open auctions
  continue past Friday** to the all-bid deadline (§8.2.1).
- In-season never reaches a crunch window: bidding is over by Sunday evening, well before Monday
  tipoff. The §8.3.2 chain does that job there, so the 1h quiet window is preseason-only.
- Most in-season auctions still close on the 24h quiet timer mid-week, long before Sunday — §8.5.2
  says as much.

Rules-doc amendments this implies (needs a league vote, not a code change): §6.4.4 gains the crunch
window and `PreseasonFinalRosterLock` as its hard deadline; §6.3.4 gains "expires the day it can
slide no further"; §8.3.2's extension chain gains "but never past the following week's roster lock".

## Backend

### entity/ (new columns/tables, enums)

Current state: `auction` has `id, kind, minimum_bid_amount, start_timestamp, soft_end_timestamp,
fixed_end_timestamp, contract_id`. `auction_bid` has `id, bid_amount, comment, auction_id,
team_user_id`. `AuctionKind` = { `InSeasonFreeAgent` ("FreeAgent"), `PreseasonVeteranAuction` }
(a commented-out `PreseasonFreeAgent` variant exists — uncomment for open-nomination/Week-1).
`soft_end_timestamp` already = start+24h, `fixed_end_timestamp` already = start+48h (NOT NULL, which
is the problem — see item 3) and nothing acts on either.

New/changed:

1. **`auction.status`** — new column + enum `AuctionStatus` { `Pending` (scheduled, not yet
   open for bids), `Open`, `Closed` (timer elapsed, awaiting `end_*_auction`), `Completed`
   (contract signed), `Expired` (closed with no bids) }. Replaces today's implicit
   "open if now < soft_end" inference. `end_veteran_auction`/`end_fa_auction` set
   `Completed`/`Expired`.
2. **`auction.close_at_timestamp`** (replaces `soft_end_timestamp`) — the instant the auction stops
   taking bids, i.e. `min(last_bid + quiet_window, all_bid_deadline)` clamped to the hard deadline.
   Mutable; written at open, on each accepted bid, on each veteran tier slide (one more day), and by
   the crunch-window sweep. Query `set_auction_close_at(auction_id, close_at, db)`. It is a stored
   column rather than a per-tick derivation so the close tick stays one indexed `close_at <= now`
   scan instead of a per-row bid lookup. **Both the close tick and `place_auction_bid` compare
   against this one value** — everything else is folded into it.
3. **`auction.all_bid_deadline_timestamp`** (replaces `fixed_end_timestamp`, now **nullable**) —
   in-season FA only: Sunday 8:00pm CT, rolled +30min by §8.3.2. NULL for veteran and preseason FA
   auctions, which have no all-bid deadline (the crunch window bounds them instead). Mutable —
   `roll_auction_all_bid_deadline(auction_id, new_deadline, db)`. Nullability is the point: the old
   `fixed_end` was NOT NULL and defaulted to start+48h, which is what let it cut off live veteran
   bidding.
4. **Crunch-window entry is a bulk update, not a per-auction timer.** Crunch start is one known
   instant per league/deadline, so when a tick crosses it, shorten every open preseason auction in
   that league to `last_bid + 1h` in one statement. Nothing needs re-deriving afterwards.
5. **`auction.minimum_bid_amount`** — already present; the tier slide-down (§6.3.4) mutates it.
   Add `update_auction_minimum_bid(auction_id, new_min, db)`.
6. **`auction_schedule`** (new table) — drives §6.3 veteran-auction release + tiers. Columns:
   `id, league_id, end_of_season_year, player_id, scheduled_release_date (NaiveDate),
   nomination_rank (i16, NULL for open-noms), min_bid_tier (i16), is_rfa_week (bool)`. One row
   per pooled player. `min_bid_tier` indexes into:
7. **`min_bid_tier_config`** (new table or JSON column on `league` season config) — ordered tiers
   `(tier_index i16, min_bid_amount i16)`. Set per-season (§6.3.6). The slide rule (§6.3.4-.5)
   only ever moves a player's auction `min_bid_amount` down to the *next* tier's value.
8. **`auction.is_rfa` / link to RFA flow** — not new here; RFA-specific resolution (48h
   raise/match, pick compensation) is **spec 03**. This engine only needs to know an auction is
   RFA-restricted so it can reject the original owner's bids and route close → spec 03 instead of
   straight signing. Add `auction.original_owner_team_id (Option<i64>)` (NULL except RFA/UFA).
9. **`auction_bid` invalid-bid record** — to honor §6.4.1 "null and void" auditably, add
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
   - Load auction; `ensure!(status == Open)`; `ensure!(now < close_at)`. One comparison — the
     all-bid deadline and the hard deadline are both already folded into `close_at`.
   - **RFA guard:** `ensure!(auction.original_owner_team_id != Some(bidder_team_id))` (§6.2.2.3 /
     §15.3.1).
   - **Increment + opening rules (§6.4.2-.3, §8.3.3-.4):** if no prior bid,
     `ensure!(bid_amount >= auction.minimum_bid_amount)`; else
     `ensure!(bid_amount >= latest_bid.bid_amount + 1)`. (Today `insert_auction_bid` only enforces
     `> previous`; tighten to `>= prev + $1` and reuse it.)
   - **§6.4.1 validity check** — `validate_bid_cap_and_roster(...)` below. On failure → `bail!`
     (the previous bid stays winning; "null and void"). This is the subtle part.
   - On success: `auction_queries::insert_auction_bid(...)`, then for in-season FA apply the §8.3.2
     roll (two trigger widths — see Timing rules) to `all_bid_deadline_timestamp`, then
     `set_auction_close_at(auction_id, min(now + quiet_window, all_bid_deadline, hard_deadline))`.
     Roll first, recompute `close_at` second, or the bid that triggers an extension closes the
     auction on the pre-roll deadline. The §6.3.4 tier slide is **not** here — it fires on *non*-bid
     at a daily tick (see scheduler).

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
   `is_rfa_week`, the ranked players get `nomination_rank` + staggered `scheduled_release_date`,
   the rest are open-nomination (rank NULL). Tier assignment writes `min_bid_tier`. UFA/RFA min bid
   = their carry salary (§15.3.1 RFA = 4th-year salary). The ranking is an import input — accept a
   ranked `Vec<player_id>` argument; do not scrape here. In practice that list is the **top 200** by
   the previous season's in-season stats, usually from FantasyPros (§6.3.2 still says top 150 from
   ESPN/Yahoo/Rotowire — one of the rules amendments above). The code is length-agnostic, so this is
   an input-data concern, not a schema one.

4. **Open a scheduled auction** — `open_scheduled_auction(auction_schedule_row, now, db)`:
   creates the `auction` (`start_new_auction_for_nba_player` already exists, generic over
   `AuctionKind`) with `status=Open`, `minimum_bid_amount` from the tier, and
   `original_owner_team_id` for RFA/UFA. Fired by the daily release tick (scheduler).

5. **Tier slide-down (§6.3.4-.5)** — `slide_unbid_auctions_down_a_tier(league_id, season, now,
   db)`: for each `Open` veteran auction with **zero bids** at the daily tick, set
   `minimum_bid_amount = next_lower_tier.min_bid_amount` and push `close_at` a day out. Per §6.3.5,
   sliding a player into a tier does **not** push the existing last player of that tier further down
   — so the slide is a pure per-auction lookup of the next configured tier value, never a cascade.
   When there is no lower tier the auction has run out of clock: expire it (player → $1 FA). This
   loop is the *only* thing that ends an unbid veteran auction, so the slide and the close tick must
   not race — see the ordering note in the scheduler section.

6. **FA opening-bid min (§8.3.3)** — when opening a *new* FA auction (preseason open-nomination,
   Week 1, or in-season): min opening bid = $1, **unless** the player was
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

- **Per-minute close tick:** find `Open` auctions with `close_at <= now` → call
  `end_veteran_auction` / `end_fa_auction`. RFA closes route to spec 03 instead of signing.
- **Crunch-window entry (preseason only):** when a tick crosses a league's crunch start (hard
  deadline − 24h, moved forward to 8:00am CT if that lands 00:00–08:00), shorten every open veteran /
  preseason-FA auction in that league to `last_bid + 1h` (clamped). One bulk update per league per
  season, guarded so it can't re-fire.
- **Daily release tick (veteran, §6.3.3):** `open_scheduled_auction` for rows whose
  `scheduled_release_date <= today`, then `slide_unbid_auctions_down_a_tier` for unbid open
  auctions. **Ordering matters:** the slide must run *before* the close tick in a given tick, or an
  unbid auction whose `close_at` just lapsed gets expired by the close tick before the slide can
  drop it a tier — which silently disables the whole tier ladder.
- **Weekly FA deadlines (§8.2):** the opening-bid deadline (Fri 11:59pm CT) gates the *nominate*
  path only — `open_*_fa_auction` refuses after it, `place_auction_bid` does not. Bids run to the
  all-bid deadline (Sun 8pm CT + §8.3.2 rolls), itself clamped to the next `InSeasonRosterLock`.
  `DeadlineKind::FreeAgentAuctionEnd` is the separate season-level FA freeze (§8.1.3);
  `Week1FreeAgentAuctionStart/End` bound the Week-1 auction.
- **The dead close condition.** An earlier draft closed on `(now > soft_end AND now > last_bid + 1h)
  OR now > fixed_end`. The 1h term was vacuous: with `soft_end` always `last_bid + 24h`, `soft_end <=
  now` already implies 24h of quiet, so the conjunction could never change an outcome. Its unit tests
  only passed by hand-feeding `soft_end`/`last_bid` pairs production cannot produce. Replaced by the
  single `close_at` comparison.

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
  current winner, min bid, time-to-close (`close_at`). Veteran mode groups by
  `scheduled_release_date`; FA mode shows the nomination deadline alongside `close_at`.
- **Auction detail / bid panel:** bid history (from `auction.get_bids`), a react-hook-form bid
  input (validate `>= min` / `>= current + 1` client-side; server is source of truth), comment
  field. Disable bid if RFA-original-owner.
- **Live bid display:** urql subscription (or 5s poll) on `auctionUpdated`; optimistic bid then
  reconcile; toast the null-and-void rejection reason.
- **Deadline countdowns:** the `close_at` timer is the one that matters, so lead with it. In-season
  also show `all_bid_deadline` when it is the binding one, with a visible +30min bump when §8.3.2
  fires (§8.5 is a bidding-war rule; owners need to see the clock move). Preseason: a crunch-mode
  marker once the window opens, since a fresh bid's reprieve drops from 24h to 1h and bidders need to
  know before they walk away. Render all times in CT (league tz) regardless of viewer locale.
- **Committed-cap meter:** show `team_current_salary + sum(my winning bids)` vs cap so the bidder
  sees the §6.4.1 headroom before a bid is rejected.

## Edge cases & open questions

- **§6.4.1 self-counting:** when re-bidding on an auction you already lead, the new amount
  *replaces* the old in the committed total — easy off-by-one. Cover with a test.
- **Tier slide vs a fresh open:** a player opened today at tier N should not also slide same-day;
  slide only auctions that have been open ≥1 daily tick with no bids. Define the exact tick offset
  in spec 05.
- **Tier slide vs the close tick:** with the slide as the only clock for an unbid veteran auction,
  a close tick that runs first will expire the auction at the moment it becomes slide-eligible. Slide
  first, close second, and cover it with a test that walks one unbid auction down every tier.
- **No-bid veteran auction** expires at the bottom of the tier ladder, not on a fixed date. FA
  auctions can't hit this path at all (the nomination is a bid), but the expire branch stays as the
  defensive default.
- **RFA hand-off:** close of an RFA auction must NOT sign immediately — it enters the 48h
  raise/match flow (spec 03). This engine sets `Closed`; spec 03 transitions to `Completed`.
- **Concurrency:** two bids racing on the same auction — wrap read-latest-bid + insert in the txn
  and rely on `>= prev + 1` to reject the loser; consider a row lock / unique-ish guard. Decide.
- **Resolved (2026-07-27):** the Friday opening-bid deadline freezes only *new* auctions; open ones
  keep taking bids to the all-bid deadline (§8.2.1). Gate `open_*_fa_auction` on Friday, and
  `place_auction_bid` on `close_at` alone.
- **Resolved (2026-07-27):** §8.2.2's Sunday 8pm all-bid deadline and §8.3.2's 30-min extension chain
  **stay** as written, for in-season only. The next `InSeasonRosterLock` is their backstop, not a
  replacement — an extension chain may not run past it.
- **Resolved (2026-07-27):** the hard deadline is absolute in every mode — clamp `close_at` to it
  rather than letting a late bid buy time past the roster lock.
- **§8.3.2's two trigger widths** (60 min before the original deadline, 30 min inside each extension)
  are the subtle part; a flat 30-min trigger silently breaks §8.5's first bid. Test the worked example
  from an 8:00pm deadline, not an 8:30pm one — the latter passes either way.
- **Preseason FA auction is a third mode, still unbuilt.** `AuctionKind::PreseasonFreeAgent` is
  commented out. It follows the veteran auction's rules except: open nomination (no schedule, no
  tiers), no no-bid expiry, and `PreseasonFinalRosterLock` as its hard deadline. It **is** cap- and
  roster-gated like the veteran auction (§6.4.1), so it must be added to
  `requires_cap_and_roster_check` — its cap is the post-auction $210 (§4.2.2), which
  `deadline::get_salary_cap` already returns for that window.
- **Resolved (2026-07-30):** the per-season ranked list + tier config are entered by the commissioner
  through `setVeteranAuctionRanking` / `setVeteranAuctionMinBidTiers`, matching the existing
  commissioner mutations rather than import-data (this is recurring human input, not a one-off
  historical backfill). Each replaces the season's rows, so re-entry is idempotent.
  `VETERAN_AUCTION_PLAYERS_RELEASED_PER_DAY` stays a constant: there is no per-season scalar config
  table to put it in, and the value has never varied. Add one when a season needs a different count.
- **Open question:** the crunch window's 24h/8am-CT shape is an implementation choice, not a voted
  rule. Confirm it in the rules amendment, and decide whether the 24h max belongs in per-season
  league config alongside the §6.3.6 tier values rather than in `constants/`.

## Dependencies

- **[spec 04](04-ufa-rfa-discount-caps.md)** — discount caps; should land first so signed UFA/RFA
  salaries are correct.
- **[spec 05](05-deadline-scheduler-and-transaction-processor.md)** — owns the timer that fires
  close/release/deadline ticks. This spec defines the functions; 05 schedules them.
- **[spec 06](06-graphql-api-surface.md)** — auction queries/mutations/subscription wiring.
- **spec 03 (RFA resolution & compensation)** — consumes `Closed` RFA auctions for the 48h
  raise/match + draft-pick forfeit.
- **specs 09/10** — per-season pool/ranking/tier import + commissioner config entry.
