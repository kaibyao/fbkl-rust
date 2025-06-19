import { ContractKind } from '@/generated/graphql';

/** Contracts that are active on a team and take up a team's salary/cap space. */
const ACTIVE_CONTRACT_KINDS = new Set([
  ContractKind.Rookie,
  ContractKind.RookieExtension,
  ContractKind.Veteran,
]);

export const isContractActiveOnTeam = (contractKind: ContractKind) =>
  ACTIVE_CONTRACT_KINDS.has(contractKind);
