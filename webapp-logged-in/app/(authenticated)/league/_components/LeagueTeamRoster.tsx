import { FunctionComponent } from 'react';
import Card from '@mui/material/Card';
import Divider from '@mui/material/Divider';
import Stack from '@mui/material/Stack';
import Typography from '@mui/material/Typography';
import {
  ContractStatus,
  TeamForRosterListFragment,
  ContractKind,
  ContractForRosterListFragment,
} from '@/generated/graphql';
import { isContractActiveOnTeam } from '@/app/_lib/contract.utils';
import List from '@mui/material/List';
import { LeagueRosterListPlayer } from '@/app/(authenticated)/league/_components/LeagueRosterListPlayer';

interface Props {
  team: TeamForRosterListFragment;
}

export const LeagueTeamRoster: FunctionComponent<Props> = ({ team }) => {
  const { activeContracts, activeButIrContracts, rookieDevelopmentContracts } =
    partitionContracts(team.contracts);

  return (
    <Card
      sx={{
        padding: 2,
      }}
    >
      <Stack spacing={2}>
        <Stack
          direction="row"
          spacing={2}
          alignItems="center"
          justifyContent="space-between"
        >
          <Typography variant="h4">{team.name}</Typography>
          <Typography variant="h5">
            $
            {activeContracts.reduce((acc, contract) => {
              return acc + contract.salary;
            }, 0)}
            /$210
          </Typography>
        </Stack>

        <Divider />

        <Stack spacing={2}>
          <Typography variant="h5">
            Active ({activeContracts.length})
          </Typography>

          <List dense style={{ marginTop: 0 }}>
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
          </List>

          <Typography variant="h5">
            Rookie Development ({rookieDevelopmentContracts.length})
          </Typography>

          <List dense style={{ marginTop: 0 }}>
            {rookieDevelopmentContracts.map((contract) => (
              <LeagueRosterListPlayer key={contract.id} contract={contract} />
            ))}
          </List>
        </Stack>
      </Stack>
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
    activeContracts,
    activeButIrContracts,
    rookieDevelopmentContracts,
  };
}
