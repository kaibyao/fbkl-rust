# Deadline Configuration

A league commissioner user needs to be able to set the deadlines for the next end-of-season year after the current season ends.

Up until the point that the commissioner "activates" the season (which means we probably need a status column in the deadline table or a new table for storing league-season configs), they should have the ability to update deadline dates as much as they want, and deadlines should not be processed if they aren't activated.

After activation, deadlines that haven't been processed yet (because they are in the future) should still be editable.

As for the actual deadlines that need to be set:
1. PreseasonKeeper - Datetime.
2. PreseasonVeteranAuctionStart - (Integer) Days after preseason keeper, at 9am.
3. PreseasonFaAuctionStart - (Integer) days to remain up, starts automatically 3 hours after the last veteran auction has started.
4. Preseason Final roster lock - (Integer) days after the rookie draft has ended.
5. Playoffs start at week # - (Integer choice, default 21).

The rest don't need to be explicitly set because those phases of the pre-season/season just happen automatically after the previous stage has finished:
1. PreseasonRookieDraftStart can start automatically 3 days after the last veteran auction has started, at 9am.
2. Week1FreeAgentAuctionStart - Starts automatically when preseason final roster lock has been processed. Lasts until Week1RosterLock.

... But what if a commissioner needs to change a deadline date after the deadline has already been processed because it was in the past? In those cases, I think we almost need a serial "rollback" strategy...
1. Delete any auctions / transactions / contracts created as part of the deadlines being processed, from most recent deadline back to the target deadline that needs to be edited.
2. Deactivate the deadline being edited as well as subsequent deadlines (This makes me think the status field needs to be on the deadlines table. This also makes me think we might want a `processed` status so we can do rollbacks on just the deadlines that have been processed).

I think we also need to hardcode the in-season roster lock deadlines in the codebase.
