import { ContractKind } from '@/generated/enums';
import { ContractForRosterListFragment } from '@/generated/graphql';

/** Contracts that are active on a team and take up a team's salary/cap space. */
const ACTIVE_CONTRACT_KINDS = new Set<ContractKind>([
  ContractKind.Rookie,
  ContractKind.RookieExtension,
  ContractKind.Veteran,
]);

export const isContractActiveOnTeam = (contractKind: ContractKind) =>
  ACTIVE_CONTRACT_KINDS.has(contractKind);

/** How each contract kind reads in the UI. `finalYearNumber` is the last `yearNumber` the kind can carry; `null` marks the kinds that are re-signed or expire inside one preseason. */
export const CONTRACT_KIND_DISPLAY: Record<
  ContractKind,
  { abbreviation: string; label: string; finalYearNumber: number | null }
> = {
  [ContractKind.RookieDevelopment]: {
    abbreviation: 'RD',
    label: 'Rookie development',
    finalYearNumber: 3,
  },
  [ContractKind.RookieDevelopmentInternational]: {
    abbreviation: 'RDI',
    label: 'Rookie development, international',
    finalYearNumber: 3,
  },
  [ContractKind.Rookie]: {
    abbreviation: 'R',
    label: 'Rookie',
    finalYearNumber: 3,
  },
  [ContractKind.RookieExtension]: {
    abbreviation: 'R',
    label: 'Rookie extension',
    finalYearNumber: 5,
  },
  [ContractKind.RestrictedFreeAgent]: {
    abbreviation: 'RFA',
    label: 'Restricted free agent',
    finalYearNumber: null,
  },
  [ContractKind.UnrestrictedFreeAgentOriginalTeam]: {
    abbreviation: 'UFA-20%',
    label: 'Unrestricted free agent, 20% off to re-sign with you',
    finalYearNumber: null,
  },
  [ContractKind.UnrestrictedFreeAgentVeteran]: {
    abbreviation: 'UFA-10%',
    label: 'Unrestricted free agent, 10% off to re-sign with you',
    finalYearNumber: null,
  },
  [ContractKind.Veteran]: {
    abbreviation: 'V',
    label: 'Veteran',
    finalYearNumber: 3,
  },
  [ContractKind.FreeAgent]: {
    abbreviation: 'FA',
    label: 'Free agent',
    finalYearNumber: null,
  },
};

/** True when the contract does not carry into next season as it stands: it converts to another kind, is re-signed, or expires. */
export const isFinalContractYear = ({
  kind,
  yearNumber,
}: Pick<ContractForRosterListFragment, 'kind' | 'yearNumber'>) => {
  const { finalYearNumber } = CONTRACT_KIND_DISPLAY[kind];
  return finalYearNumber === null || yearNumber >= finalYearNumber;
};
