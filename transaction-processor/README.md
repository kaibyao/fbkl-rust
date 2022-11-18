# Transaction Processor

This is a placeholder document to outline the idea behind the Transaction Processor.

There are various actions that teams can make that affect the state of the league: trading, bidding/winning an auction, drafting rookies, dropping contracts, etc.

There needs to be a way to record these various things for these reasons:
* Ensure correctness of the system.
  * Based on recorded transactions only, it should be possible to re-create the state of a league, even if intermediary data (bids, comments, trade actions that aren't the final trade state) is lost.
* Provide a log for users to see.

So how do we create a transaction? The idea is to have both:
1. A running process that periodically (I'm thinking once per minute) checks various tables and acts on them when deadlines are reached.
2. An importable module that processes the necessary transaction.

These are the actions that need to be processed.
* It's been 24H since the last bid on an auction. Record that an auction has ended and make the relevant contract + team updates. (Transaction: AuctionDone. TeamUpdate: AddViaAuction)
* Record that a rookie has been drafted. Make the relevant contract + team update. (T: TeamUpdate. TU: AddViaRookieDraft)
* Record that a team dropped a player. (T: TeamUpdate. TU: Drop)
* Record that two teams completed a trade. (T: Trade. TU: TradedAway/AddViaTrade)
* Record that a team has activated a rookie. (T: TeamUpdate. TU: ActivateRookie)
* Record that a team is moving a contact to/from IR. (T: TeamUpdate. TU: ToIR/FromIR)
* Record that a team has changed their settings. (T: TeamUpdate)
  * Should we go one level deeper and make each kind of setting update its own transaction? IE "Team has added new owner" should be its own transaction vs just rolled into a team setting update. This might just be a UI problem. Or an intermediary/GraphQL problem, not a data structure problem.

^ Going along with that, a user is probably going to want to see just the changes that are relevant for their team, so it makes sense to have stuff live in both Transaction and TeamUpdateContract.
