import { FunctionComponent, ReactNode } from 'react';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent } from '@/components/ui/card';
import { Progress } from '@/components/ui/progress';
import { Stack, StackGap } from '@/components/ui/stack';
import { TermTip } from '@/components/ui/term-tip';
import { Typography, TypographyVariant } from '@/components/ui/typography';
import { ContractKind } from '@/generated/enums';
import { CONTRACT_KIND_DISPLAY } from '@/lib/contract.utils';
import {
  LEAGUE_RULES_SECTION_ORDER,
  LEAGUE_RULES_SECTION_TITLE,
  LeagueRulesSection,
} from '@/lib/league-rules';
import { TERMS } from '@/lib/terms';

// The kinds the Contracts section prints, in the order a player passes through them.
const CONTRACT_KINDS_IN_CAREER_ORDER: ContractKind[] = [
  ContractKind.RookieDevelopment,
  ContractKind.Rookie,
  ContractKind.Veteran,
  ContractKind.RestrictedFreeAgent,
  ContractKind.UnrestrictedFreeAgentOriginalTeam,
];

const contractKindChips = (
  <div className="flex flex-wrap gap-1.5">
    {CONTRACT_KINDS_IN_CAREER_ORDER.map((kind) => {
      const { abbreviation, label, finalYearNumber } =
        CONTRACT_KIND_DISPLAY[kind];
      return (
        <TermTip
          key={kind}
          label={label}
          render={<Badge variant="secondary" className="cursor-help" />}
        >
          {finalYearNumber === null
            ? abbreviation
            : `${abbreviation} Y1/${finalYearNumber}`}
        </TermTip>
      );
    })}
  </div>
);

/**
 * Section bodies, keyed by section so a missing or extra entry is a compile error.
 *
 * Every number here is the shipped one: cap steps, auction clocks and rookie salaries come from
 * `constants/src/league_rules/config_settings.rs`, the rest from `notes/2025-08-31-rules_document.md`.
 */
const LEAGUE_RULES_SECTION_BODY: Record<LeagueRulesSection, ReactNode> = {
  [LeagueRulesSection.Contracts]: (
    <>
      <Typography variant={TypographyVariant.Muted}>
        A player on your roster is a contract: a salary, a year inside a fixed
        span, and a kind that decides what happens to him next season. The
        roster prints that kind as a chip.
      </Typography>
      {contractKindChips}
      <ul>
        <li>
          Keep a player and his salary rises 20% of his current salary, rounded
          up. Rookie development salaries never rise.
        </li>
        <li>
          A player signed in an auction or in-season free agency runs three
          years, then leaves as an unrestricted free agent. You keep a
          re-signing discount on him.
        </li>
        <li>
          A drafted rookie sits on rookie development for up to three years,
          becomes a first-year rookie contract the moment you activate him, and
          becomes a restricted free agent after his third rookie season. Re-sign
          him there and he can run to five years.
        </li>
        <li>
          At the keeper deadline you may keep at most 14 players totalling at
          most $100. Rookie development players do not count against either
          limit.
        </li>
      </ul>
    </>
  ),
  [LeagueRulesSection.SalaryCap]: (
    <>
      <Typography variant={TypographyVariant.Muted}>
        The sum of your salaries must sit at or under the cap at all times. The
        cap is not one number: it steps up twice across the season.
      </Typography>
      <Progress
        value={186}
        max={210}
        label="Example team: $186 of a $210 cap used"
      />
      <Typography variant={TypographyVariant.MutedSm}>
        $186 of $210 used. The roster prints the same bar for your team.
      </Typography>
      <ul>
        <li>
          <b>$200</b> from the keeper deadline through the veteran auction and
          rookie draft.
        </li>
        <li>
          <b>$210</b> once both finish, which is what pays for activating rookie
          development players.
        </li>
        <li>
          <b>$230</b> once in-season pickups freeze, and it stays there through
          the playoffs.
        </li>
        <li>
          No cap at all between the end of the playoffs and the keeper deadline.
        </li>
        <li>
          Drop a player after the keeper deadline and your cap falls by 20% of
          his salary, rounded up, for the rest of the season. Each drop is
          charged on its own, and rookie development drops are free.
        </li>
      </ul>
    </>
  ),
  [LeagueRulesSection.Auctions]: (
    <>
      <Typography variant={TypographyVariant.Muted}>
        Two auctions with two different clocks. Bids move in $1 steps in both,
        and a bid only counts if you have the cap and the roster room to pay it.
      </Typography>
      <Typography variant={TypographyVariant.Heading4}>
        Veteran auction, preseason
      </Typography>
      <ul>
        <li>
          The first 7 days are restricted free agents only. After that, 15
          players are released for bidding each day.
        </li>
        <li>
          An auction closes 24 hours after its last bid. In the last day before
          the hard deadline that quiet window shrinks to 1 hour, and it never
          opens earlier than 8am CT.
        </li>
        <li>
          Win a <TermTip label={TERMS.RFA}>RFA</TermTip> and you have 48 hours
          to raise your own bid; his old team then has 48 hours to match it at
          10% off or take a draft pick from you instead.
        </li>
      </ul>
      <Typography variant={TypographyVariant.Heading4}>
        In-season free agency
      </Typography>
      <ul>
        <li>Open an auction on a free agent by Friday 11:59pm CT.</li>
        <li>Bid on any open auction until Sunday 8pm CT.</li>
        <li>
          A bid in the hour before that Sunday deadline pushes it out 30
          minutes, and every later bid inside 30 minutes pushes it out another
          30.
        </li>
        <li>
          The opening bid is $1, unless he was owned earlier this season, in
          which case it is his salary at the time he was dropped.
        </li>
      </ul>
    </>
  ),
  [LeagueRulesSection.RookieDevelopment]: (
    <>
      <Typography variant={TypographyVariant.Muted}>
        The rookie draft is 5 rounds, and every pick has a fixed salary set by
        its round.
      </Typography>
      <div className="flex flex-wrap gap-1.5">
        <Badge variant="secondary">Rd 1 · $4</Badge>
        <Badge variant="secondary">Rd 2 · $3</Badge>
        <Badge variant="secondary">Rd 3 · $2</Badge>
        <Badge variant="secondary">Rd 4 · $1</Badge>
        <Badge variant="secondary">Rd 5 · $1</Badge>
      </div>
      <ul>
        <li>
          The six teams that miss the playoffs enter a lottery for the first six
          picks, holding 6, 5, 4, 3, 2 and 1 balls, worst record to best.
        </li>
        <li>
          Drafted rookies land on rookie development. They cost you no cap and
          no roster slot, their salary never rises, and dropping them costs no
          penalty.
        </li>
        <li>
          In season you may hold 6 rookie development players plus 1
          international one.
        </li>
        <li>
          Activating a player converts him to a first-year rookie contract at
          the same salary, needs the cap and roster room to fit him, and cannot
          be undone.
        </li>
      </ul>
    </>
  ),
  [LeagueRulesSection.Ir]: (
    <>
      <Typography variant={TypographyVariant.Muted}>
        One <TermTip label={TERMS.IR}>IR</TermTip> slot per team in season. His
        salary sits off your books while he is in it, which is the whole point
        of the slot.
      </Typography>
      <ul>
        <li>
          Only a player on the real NBA injured list qualifies, and the league
          site decides that.
        </li>
        <li>
          A player you acquire in season has to fit on the 22-man active roster
          first. You can move him to IR after that, not instead.
        </li>
        <li>
          Bringing him back needs cap and roster room, exactly as if you had
          just signed him.
        </li>
        <li>
          You may drop him straight from IR, but the 20% drop penalty still
          applies.
        </li>
      </ul>
    </>
  ),
  [LeagueRulesSection.WeeklyMoves]: (
    <>
      <Typography variant={TypographyVariant.Muted}>
        Every in-season move happens inside one week: adds, drops, IR moves,
        trades and activations. Within that week you may order them however you
        like.
      </Typography>
      <ul>
        <li>
          Your roster may be over the cap or over the limit mid-week. It has to
          be legal again before Monday roster lock.
        </li>
        <li>
          In-season limits are 22 active players, 1 IR, 6 rookie development and
          1 international rookie development.
        </li>
        <li>
          Offseason the limit is 32 players with no IR slot, so the roster cut
          deadline before the season is where most teams pay a drop penalty.
        </li>
        <li>
          Players won in the same week must all be added legally before any of
          them can be dropped.
        </li>
      </ul>
    </>
  ),
};

/**
 * Help tier 3: the league rules in read mode. Contents down the left, short scannable sections on
 * the right, each written with the app's own chips and bars so reading the rules teaches the
 * interface. Every section carries its `LeagueRulesSection` id as an anchor for "Learn more" links.
 */
export const LeagueRules: FunctionComponent = () => {
  return (
    <div className="grid gap-6 md:grid-cols-[190px_1fr] md:items-start">
      <nav aria-label="League rules contents" className="md:sticky md:top-4">
        <Typography variant={TypographyVariant.SectionLabel}>
          Contents
        </Typography>
        <ul className="mt-2 grid gap-0.5">
          {LEAGUE_RULES_SECTION_ORDER.map((section) => (
            <li key={section}>
              <a
                href={`#${section}`}
                className="block rounded-md px-2 py-1.5 text-sm text-muted-foreground hover:bg-card hover:text-foreground"
              >
                {LEAGUE_RULES_SECTION_TITLE[section]}
              </a>
            </li>
          ))}
        </ul>
      </nav>

      <Stack gap={StackGap.Md}>
        <Typography variant={TypographyVariant.Heading1}>
          League rules
        </Typography>
        {LEAGUE_RULES_SECTION_ORDER.map((section) => (
          // scroll-mt keeps the heading clear of the sticky page header when a "Learn more" jumps here.
          <Card key={section} id={section} className="scroll-mt-20">
            <CardContent>
              <Stack
                gap={StackGap.Sm}
                className="[&_ul]:grid [&_ul]:list-disc [&_ul]:gap-1.5 [&_ul]:pl-5 [&_ul]:text-sm [&_ul]:text-muted-foreground"
              >
                <Typography variant={TypographyVariant.Heading2}>
                  {LEAGUE_RULES_SECTION_TITLE[section]}
                </Typography>
                {LEAGUE_RULES_SECTION_BODY[section]}
              </Stack>
            </CardContent>
          </Card>
        ))}
      </Stack>
    </div>
  );
};
