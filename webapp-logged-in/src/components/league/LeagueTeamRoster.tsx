import { FunctionComponent } from 'react';
import { LeagueRosterListPlayer } from '@/components/league/LeagueRosterListPlayer';
import { Card, CardContent } from '@/components/ui/card';
import { Separator } from '@/components/ui/separator';
import { ContractKind, ContractStatus } from '@/generated/enums';
import {
  ContractForRosterListFragment,
  TeamForRosterListFragment,
} from '@/generated/graphql';
import { isContractActiveOnTeam } from '@/lib/contract.utils';

interface Props {
  team: TeamForRosterListFragment;
}

export const LeagueTeamRoster: FunctionComponent<Props> = ({ team }) => {
  const { activeContracts, activeButIrContracts, rookieDevelopmentContracts } =
    partitionContracts(team.contracts);

  const activeSalary = activeContracts.reduce(
    (acc, contract) => acc + contract.salary,
    0,
  );

  return (
    <Card>
      <CardContent className="flex flex-col gap-3">
        <div className="flex items-center justify-between gap-2">
          <h2 className="font-heading text-base font-bold">{team.name}</h2>
          <span className="font-heading text-sm font-bold tabular-nums">
            ${activeSalary}
            <span className="text-muted-foreground">
              /${team.salaryCap.salaryCap}
            </span>
          </span>
        </div>

        <Separator />

        <div className="flex flex-col gap-2">
          <h3 className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
            Active ({activeContracts.length})
          </h3>

          <ul className="flex flex-col">
            {activeContracts.map((contract) => (
              <LeagueRosterListPlayer key={contract.id} contract={contract} />
            ))}
            {activeButIrContracts.map((contract) => (
              <LeagueRosterListPlayer
                key={contract.id}
                contract={contract}
                isIr
              />
            ))}
          </ul>

          <h3 className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
            Rookie Development ({rookieDevelopmentContracts.length})
          </h3>

          <ul className="flex flex-col">
            {rookieDevelopmentContracts.map((contract) => (
              <LeagueRosterListPlayer key={contract.id} contract={contract} />
            ))}
          </ul>
        </div>
      </CardContent>
    </Card>
  );
};

function partitionContracts(contracts: ContractForRosterListFragment[]): {
  activeContracts: ContractForRosterListFragment[];
  activeButIrContracts: ContractForRosterListFragment[];
  rookieDevelopmentContracts: ContractForRosterListFragment[];
} {
  const activeContracts: ContractForRosterListFragment[] = [];
  const activeButIrContracts: ContractForRosterListFragment[] = [];
  const rookieDevelopmentContracts: ContractForRosterListFragment[] = [];

  contracts.forEach((contract) => {
    // Rookie development contracts (including international)
    if (
      contract.kind === ContractKind.RookieDevelopment ||
      contract.kind === ContractKind.RookieDevelopmentInternational
    ) {
      rookieDevelopmentContracts.push(contract);
    }
    // Active contracts on team
    else if (
      contract.status === ContractStatus.Active &&
      isContractActiveOnTeam(contract.kind)
    ) {
      if (contract.isIr) {
        activeButIrContracts.push(contract);
      } else {
        activeContracts.push(contract);
      }
    }
  });

  return {
    activeContracts: activeContracts.sort(
      (
        a,
        b, // contract value
      ) =>
        b.salary - a.salary ||
        // year number
        b.yearNumber - a.yearNumber ||
        // name
        a.leagueOrRealPlayer.name.localeCompare(b.leagueOrRealPlayer.name),
    ),
    activeButIrContracts,
    rookieDevelopmentContracts: rookieDevelopmentContracts.sort((a, b) => {
      // International players are at the end
      if (a.kind === ContractKind.RookieDevelopmentInternational) {
        return 1;
      }
      if (b.kind === ContractKind.RookieDevelopmentInternational) {
        return -1;
      }
      return (
        // Sort by year number, then by name
        b.yearNumber - a.yearNumber ||
        a.leagueOrRealPlayer.name.localeCompare(b.leagueOrRealPlayer.name)
      );
    }),
  };
}
