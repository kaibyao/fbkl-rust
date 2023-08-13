# How to structure & trade draft pick options

Do draft pick protections (options) always stay attached to their designated draft pick, or can they be traded separate from their picks?

The naiive / easy thing to implement is to force a protection to always be owned by the team that owns its pick. But in the real world, I think there have been circumstances in which a protection has moved separately from its attached pick.

## Scenarios

### Example 1:

1. Team A trades their 1st rd pick (Let's call it A-1) that's top-10 protected to Team B.
2. Later, Team B makes another trade to Team A, where they trade some other assets in order to remove the top-10 protection.

This is probably the simplest form of attaching options/protections to a draft pick. A Top-X option is made that is owned by a team but targets 1 or more draft picks. Later, that option is removed from the draft pick in a different trade.

### Example 2:

1. Team A trades a 1st rd draft pick swap with Team B. Technically, this means that Team A trades pick A-1 to Team B for pick B-1, and attaches an option to A-1 that stipulates that the owner of A-1 can trade A-1 for B-1 at the time the draft starts, if B-1 is the earlier pick.
2. Team A later trades pick B-1 to Team C.
3. At the time of the draft, it turns out pick B-1 is earlier than A-1.
4. Team B swaps pick A-1 for B-1 with Team C. Team C ends up with A-1.

In actuality, the option targets both draft picks A-1 and B-1, even though the option "belongs" to the team that owns A-1.

This is a bit more complex, in that an option targets 2 draft picks, but is owned by a team.

### Example 3:

1. Team A owns a 1st round pick that they had traded for from Team B (B-1).
2. Team A trades pick B-1 to Team C and attaches the following protections around it:
    1. B-1 is Top 10-protected in `Current Year`.
    2. If B-1 ends up in the Top-10 in `Current Year`, then the owner of B-1 receives a Top 10-protected 1st round pick in `Current Year + 1` from Team A.
    3. If A-1 in `Current Year + 1` ends up in the Top-10, then the owner of that draft pick receives Team A's 2nd round draft pick instead (A-2).
3. Lo and behold, Team A gets lucky and B-1 in `Current Year` and A-1 in `Current Year + 1` end up in the first 10 draft picks. Team A acquires B-1 in `Current Year` from Team C, A-1 in `Current Year + 1` from Team C, and trades A-2 in `Current Year + 1` to Team C.

This is probably about as complex as it gets for fantasy purposes. An option is attached to a pick not owned by the team that owns the option but owns the pick, with conditionals attached to it such that options are attached to future draft picks owned by the team that owned the original draft pick.

## Thoughts

This makes me realize that options can get pretty creative, and it would be increasingly difficult to support the many ways in which users might want to attach options to draft picks.

That said, my original thought of having a pick always move with its attached draft pick in a trade was too naiive, and would not support the ways in which teams would want to configure draft pick options.

I think the correct thing to do is to implement the following:
1. Allow a draft pick option to target 1 or more draft picks.
2. Introduce a `Nullified` status (in case a protection is removed) to the Draft Pick option.
3. Options wouldn't be something traded between teams (after initial creation). Rather, they exist in their own "space" within a League season. Any team that owns a draft pick that's targeted by an option would see that option's description on the draft pick.
4. Draft picks options would have a string clause that describes the option's effect. It's not really feasible to create a structure around the infinite ways in which teams can protect draft picks.
5. Introduce a new data structure/schema for `Draft Pick Option Amendment`, which:
    1. Is something that can be traded/accepted in a trade.
    2. Can target a draft pick option that: A) targets any draft pick currently owned by any team involved in the trade, and B) cannot target a draft pick option that targets a draft pick owned by a team that's not involved in the trade.
    3. Has the following Amendment Types:
        * `Nullify` / `Removal` / `Cancel` - Removes the effect of an option.
        * `Amend` - Changes the effect of the targeted option or adds to it.
    4. Also similarly can't be traded after its initial creation and exists in the same space as the draft pick option(s) to which it is attached.

This also means that I cannot continue in fleshing out trade processing until I implement the above.

### Post-implementation updates

Argh, this is what happens when I think about these things late at night and haven't given an idea enough time to marinate.

I just realized that I don't need the structure around amendments, because I could just add another option to a draft pick instead. IE, if a draft pick already has an option, I can just add another option in a trade that says "Cancel / change the original option".

Welp, time to remove the usage of "amendments" and change its callsites to create a new draft pick option instead. Sooooo much simpler.
