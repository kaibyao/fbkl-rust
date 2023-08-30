The thing that's hard about transactions is when they are made vs when they go into effect. 2 owners might trade players on a Wednesday, but it's not until the following Monday (during roster legalization, which happens at the start of the first game of that day) that their rosters are fully updated.

And how do we display the changes caused by a transaction during that interim period between when the transaction occurs and the following roster legalization?

Now that we've identified the two main problems (how to reason about the interim period and how to display), let's hash it out:

Types of transactions:
* Keepers set
* Rookie drafted
* Contracts traded between 2 or more teams
* A UFA/RFA/Veteran/FA auction being won
* Team update (config or roster), possibly as a result of one of the above transactions.
  * Which makes me wonder whether we need a parent-child relationship between certain kinds of transactions
  * Because setting keepers will cause players to be dropped (team update).
  * Drafting a rookie adds an RD contract to a team (team update).
  * Trades also change the contracts tied to a team (team update).
  * Winning an auction adds a player contract to a team (team update).
  * Owners can also change (config change), the team name can change (config change), and owners can choose to drop a contract (team update).
  * Switching contracts in IR status is also a team update.

Which makes me wonder if team_update and transaction is the parent->child relationship that we should use. Like, maybe a transaction can be the hub that connects the reason for a transaction (keeper/rookiedraft/trade/auction/drop/IR) to 1 or more team updates.

Speaking of IR, should it be a flag on the contract, or an FK id on the team?
* Probably keep is on contract and have validation for it on a team update.

And which date do I use? I feel like we almost need a different table for recording weekly roster legalizations and have the transactions reference one. That way, legalization contains the date that changes take effect, while transaction dates match with when they actually happened.
* Back to this debate about calculating + caching vs storing as table data. I think caching makes more sense, as the data is constantly calculated w/ the same parameters (date-to-weeks)
* Then again, what if league configuration were to change mid-season? You'd want the original weekly start dates to be stored in persistent storage to keep track of history.
* Pre-generating weekly legalization dates and storing them in the DB is also easier. If configuration were to change mid-season, we'd just update the legalization records for that league that are after the current date.

So now that we've established that we need a `weekly_team_legalization` table of some sort to mark a week's legalization date & time, we should then introduce an FK column on transactions that point to the legalization table.

On the topic of storing dates for when things should happen, how should we store things like pre-season keeper lock, rookie draft dates, etc? Should the legalization table be larger in scope, with `legalization` as an enum value? Or should this other thing be a different table?
* My vote for now is on a different table, as having weekly roster lock being targeted by a transaction makes sense.
* Then again, this kinda breaks when it comes to transactions made during pre-season, as those don't wait on a legalization date to take effect (they take effect immediately). I guess the FK column should be made nullable, along with another column that flags a transaction as made outside the bounds of a season. There should be a validation check that enforces that a transaction has this FK when done in-season.
* A `deadline` table might make sense...
  * `datetime`
  * `deadline_type`
    * `preseason_keeper`
    * `veteran_auction_rfa_start`
    * `veteran_auction_fa_start`
    * `rookie_draft_start`
    * `in_season_legalization`
    * `fa_auction_end`
    * `cap_increase`
    * `trade_deadline`
    * `season_end`
  * `name`
  * `league_id`
  * `end_of_season_year`

So to summarize:
* A transaction is an action that happens in a league that changes the contracts owned by 1 or more teams.
* A transaction can have many team updates (team_update should point to transaction?).
* A transaction can be of types:
  * Keepers set during pre-season (there is a date to point to)
    * Pre-season keeper deadline should be a type of legalization?
  * Rookie drafted
  * Contracts/picks traded between 2 or more teams
  * A UFA/RFA/Veteran/FA auction being won
  * Team update (config or roster) manually made by an owner, which can be:
    * Dropping a player contract.
    * Moving a player contract to IR or activating a player contract out of IR.
    * Adding an owner.
    * Dropping an owner.
    * Changing team name.

How to handle FK relationships?
* `team_update` should have a FK column pointing to transaction (Because a transaction can involve multiple team_updates).
* I feel like the ergonomics are easier (even though this doesn't follow the same convention as `team_update`->`transaction`) if `transaction` had nullable FK columns (validated via code) pointing to:
  * `auction` (auction has_one transaction, transaction has_one auction)
  * `deadline` (deadline has_many transactions, transaction belongs_to deadline)
  * `rookie_draft_selection` (has_one, has_one)
  * `trade` (has_one, has_one)

* There should also be a `transaction_type` enum with the following values:
  * `auction_done`
  * `keeper_deadline`
  * `rookie_draft_selection`
  * `trade`
  * `team_update_drop_contract`
  * `team_update_to_ir`
  * `team_update_from_ir`
  * `team_update_config_change`

In regards to how to do the "effective date" of a transaction... if we can confirm that all transactions are effective based on specific deadlines (keeper deadline, roster legalization), then we can just have it point to a deadline record. Otherwise it'd need its own custom date & time column (or we just use `created_at`). OK so, `config_change` is the only transaction type that's not tied to a deadline datetime, so we should make the deadline FK column nullable, and validate it to be required for every `transaction_type` except `config_change`.

Another option is to just... not generate a transaction on config changes. Then `team_update.transaction_id` becomes nullable and `transaction.deadline_id` becomes non-nullable. Need validation based on TeamUpdateType. I like this approach more.

## When do transactions take effect?

I'm thinking that transactions and deadlines take effect at different times.

Here's the list of all transaction types and when they'd happen:
* `Trade` - Immediately.
* `AuctionDone` - Immediately.
* `PreseasonKeeper` - At PreSeason Keeper deadline (special case).
* `RookieDraftSelection` - Immediately.
* `TeamUpdateDropContract` - At next weekly lock deadline.
* `TeamUpdateToIr` - At next weekly lock deadline.
* `TeamUpdateFromIr` - At next weekly lock deadline.
* `RookieContractActivation` - At next weekly lock deadline.
* `TeamUpdateConfigChange` - Immediately.

For the ones that happen immediately, there would be a separate deadline validation that runs to make a team's roster is legal for a deadline.

## UPDATE 2023-08-26

Wondering if we actually need to tie transactions to deadlines. Thinking more high level, deadlines are just dates by which certain actions have to happen. Transactions are changes that are made to a team. My previous thoughts on transactions happening at a different time than the changes effected by the transactions might be incorrect... In a simpler model, all transactions happen immediately, while separate roster lock validation and processing happens when a deadline is reached.

Are there any downsides to doing things this way?
* What if people want to keep the flexibility they have now with choosing how to legalize their roster at the end of the week? Right now, while it's true that a roster has to be legal at the time of a trade, you don't have to decide HOW to make it legal until the end of the week. With this system, you need to choose which players to drop in order to make your roster legal for a trade, during the time of the trade.
* Probably still good to link the team_updates / transactions to the deadline, so we can know if a roster lock isn't completed in a league, which team updates haven't been processed.
* Then again, couldn't we just use error logging?
