So how should the Keepers flow work?

Keepers happen after existing team contracts are advanced by 1 year, and owners must choose who in their roster they wish to keep or drop.

There is a limit of $100 salary and 14 non-(RD|RDI|RFA|UFA) players.

Players will ideally choose via UI.

On the server, I can think of 2 ways to implement:

## #1: Use team update w/ deadline
Each team would have a team update pointing to a deadline of type PreseasonKeeper (via transaction -> deadline).

Updates to the keeper list would update the team update record.

## #2: Use a Keeper table
Each team would save their list of keepers to a table w/ contract ids and actions taken. The more I think about it, the more I think this is the wrong way to do it. If I create a table for Keepers for each team + league, do I have to create a separate table for every kind of roster change that happens in the future? The whole purpose of the deadline table is to help handle cases like these.
