/**
 * The sections of the League rules page, in reading order.
 *
 * Each member's value is the URL hash the section renders as its anchor id, so a tier-2
 * `<ExplainerNote>` "Learn more" link points at one member instead of a bare string.
 */
export enum LeagueRulesSection {
  Contracts = 'contracts',
  SalaryCap = 'salary-cap',
  Auctions = 'auctions',
  RookieDevelopment = 'rookie-development',
  Ir = 'ir',
  WeeklyMoves = 'weekly-moves',
}

/** Reading order for the table of contents and the page body. Both walk this one list. */
export const LEAGUE_RULES_SECTION_ORDER: LeagueRulesSection[] = [
  LeagueRulesSection.Contracts,
  LeagueRulesSection.SalaryCap,
  LeagueRulesSection.Auctions,
  LeagueRulesSection.RookieDevelopment,
  LeagueRulesSection.Ir,
  LeagueRulesSection.WeeklyMoves,
];

/** Section headings, keyed by section so a missing or extra entry is a compile error. */
export const LEAGUE_RULES_SECTION_TITLE: Record<LeagueRulesSection, string> = {
  [LeagueRulesSection.Contracts]: 'Contracts',
  [LeagueRulesSection.SalaryCap]: 'Salary cap',
  [LeagueRulesSection.Auctions]: 'Auctions',
  [LeagueRulesSection.RookieDevelopment]: 'Rookie development',
  [LeagueRulesSection.Ir]: 'Injured reserve',
  [LeagueRulesSection.WeeklyMoves]: 'Weekly moves',
};
