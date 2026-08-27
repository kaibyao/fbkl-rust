/**
 * The words behind each abbreviation the app prints, for `<TermTip label={TERMS.X}>`.
 *
 * Contract kinds are deliberately absent: their labels already live in `CONTRACT_KIND_DISPLAY`
 * (`lib/contract.utils.ts`), keyed by the backend `ContractKind` enum, and the roster row feeds them
 * straight into a `<TermTip>`. This table holds only the abbreviations that have no other home, and it
 * grows one entry per screen that prints a new one.
 */
export const TERMS = {
  /** Injury status on a player row. */
  GTD: 'Game-time decision: his NBA team decides whether he plays shortly before tipoff.',
  /** Chip on a roster row whose contract sits in the injured-reserve slot. */
  IR: 'Injured reserve: one slot per team, and his salary does not count against your cap while he sits in it.',
  /** Auction rule named in the League rules page. */
  RFA: 'Restricted free agent: his old team cannot bid, but it can match the winning bid at a 10% discount or take a draft pick instead.',
  /** Salary shown on a free-agent contract, which has no salary yet. */
  TBD: 'To be decided: a free agent has no salary until an auction sets one.',
  /** Stat column comparing actual production to the preseason projection. */
  VS_PROJ:
    'Versus projection: how the player performs against their preseason projection.',
} as const satisfies Record<string, string>;
