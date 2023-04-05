# So how should the Keepers flow work?

Keepers happen after existing team contracts are advanced by 1 year, and owners must choose who in their roster they wish to keep or drop.

There is a limit of $100 salary and 14 non-(RD|RDI|RFA|UFA) players.

Players will ideally choose via UI.

## How to implement the actual deadline processing?

On the server, I can think of 2 ways to implement:

### #1: Use team update w/ deadline
Each team would have a team update pointing to a deadline of type PreseasonKeeper (via transaction -> deadline).

Updates to the keeper list would update the team update record.

### #2: Use a Keeper table
Each team would save their list of keepers to a table w/ contract ids and actions taken. The more I think about it, the more I think this is the wrong way to do it. If I create a table for Keepers for each team + league, do I have to create a separate table for every kind of roster change that happens in the future? The whole purpose of the deadline table is to help handle cases like these.


So we're going with Option 1...

So what should the relation between the keeper deadline and team updates and transactions be? Does one transaction contain all the team updates for a keeper deadline, or should each team update have its own transaction that each point to the keeper deadline? I think the latter; mainly because when you're looking at the transaction log, you're expecting to see a different transaction for each team, rather than one large transaction containing every team change. From a systems / coding perspective though, it'd be easier to get all the team updates for a single transaction and then just display them as if they were separate transactions in the UI.

But then that brings philosophical questions to mind:
1. What is the nature of a transaction? Does a transaction involve multiple/all teams, or does it involve just 1 team?
2. How would we maintain / delineate custom rules that make it so that one kind of transaction only affects 1 team, but another might affect multiple or all teams?

For #1, take trades for example. A trade is a single transaction between 2 or more teams that involve multiple contracts & draft picks. You wouldn't think of a trade as each team having its own transaction. But then take the Keeper Deadline. You'd think each team is making its own transaction to finalize its roster for Keeper Deadline.

As for #2, I think you could have an enum called `TransactionAffectsNumberOfTeams` with values: `One`, `Multiple`, and `All`, and just have a `HashMap<TransactionType, TransactionAffectsNumberOfTeams>` constant that the rest of the codebase can refer to in order to determine its behavior around how to process the transaction.

It probably is better to use one large transaction containing multiple team changes when able, and use the map to determine logic.

If we were to make every transaction affect 1 team only, you'd have to figure out how to group multiple transactions together for things like trades or keeper deadline, and that feels more painful.

## How to handle UFA/RFAs?

When we do the yearly contract advancement process that happens before the Keeper period, contracts that were R3, V3, or R5 become free agent contracts, and have an auction period where the highest bidder can retain the player associated with the contract. But in the interim period where they are RFA or UFA, do they belong to a team?

I can think of 2 ways to think about this. Either:
1. The player is not associated with a team and we'd have to look at their previous contract to try to find the team that they are associated with, or
2. The player IS still associated with the team and we just don't display it as such in the UI.

I think #2 is the way to go. In the real-world, when a player enters free agency, they are still somewhat tied to their previous team in the sense that that team is the one w/ the first chance to talk to the player / make things work out. In the code base, this would be easiest to make sense of, as the RFA contract is still part of the team's roster / team updates, and it's not until another team retains them in their next contract does the original team lose the player.

## Wait, why did I make team_update_contract work this way?

I must've been really sleepy when I thought up its current implementation. Right now a team can have multiple team_update, and a team_update stores an object containing its current contract IDs as well as team settings. But there's also a team_update_contract table that stores the changes that were made to contracts in a team update.

The thing I'm noticing is that it's kinda hard to figure out what contracts are currently associated the team. If anything, I'd want team_update_contract to be team_contract instead, and have a team_update store the changes made to its contracts.
