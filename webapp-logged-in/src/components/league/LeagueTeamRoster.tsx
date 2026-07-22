import { FunctionComponent } from 'react';
import { LeagueRosterListPlayer } from '@/components/league/LeagueRosterListPlayer';
import { Card, CardContent } from '@/components/ui/card';
import { Separator } from '@/components/ui/separator';
import {
  Stack,
  StackAlign,
  StackDirection,
  StackGap,
  StackJustify,
} from '@/components/ui/stack';
import { Typography, TypographyVariant } from '@/components/ui/typography';
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
    (acc, contract) => acc + (contract.salary ?? 0),
    0,
  );

  return (
    <Card>
      <CardContent>
        <Stack gap={StackGap.Md}>
          <Stack
            direction={StackDirection.Row}
            align={StackAlign.Center}
            justify={StackJustify.Between}
            gap={StackGap.Sm}
          >
            <Typography variant={TypographyVariant.Heading2}>
              {team.name}
            </Typography>
            <Typography variant={TypographyVariant.Stat}>
              ${activeSalary}
              <Typography
                variant={TypographyVariant.InlineMuted}
                render={<span />}
              >
                /${team.salaryCap.salaryCap}
              </Typography>
            </Typography>
          </Stack>

          <Separator />

          <Stack gap={StackGap.Sm}>
            <Typography variant={TypographyVariant.SectionLabel}>
              Active ({activeContracts.length})
            </Typography>

            <Stack render={<ul />}>
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
            </Stack>

            <Typography variant={TypographyVariant.SectionLabel}>
              Rookie Development ({rookieDevelopmentContracts.length})
            </Typography>

            <Stack render={<ul />}>
              {rookieDevelopmentContracts.map((contract) => (
                <LeagueRosterListPlayer key={contract.id} contract={contract} />
              ))}
            </Stack>
          </Stack>
        </Stack>
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
        (b.salary ?? 0) - (a.salary ?? 0) ||
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
