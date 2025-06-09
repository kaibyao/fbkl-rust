import { FunctionComponent } from 'react';
import Card from '@mui/material/Card';
import Divider from '@mui/material/Divider';
import Stack from '@mui/material/Stack';
import Typography from '@mui/material/Typography';
import {
  ContractForRosterListFragmentDoc,
  TeamForRosterListFragment,
} from '@/generated/graphql';
import { useFragment } from '@/generated';

interface Props {
  team: TeamForRosterListFragment;
}
export const LeagueTeamRoster: FunctionComponent<Props> = ({ team }) => {
  return (
    <Card
      sx={{
        padding: 2,
      }}
    >
      <Stack spacing={2}>
        <Stack direction="row" spacing={2} justifyContent="space-between">
          <Typography variant="h4">{team.name}</Typography>
          <Stack>
            <Typography variant="body2">
              Roster size: {team.contracts.length} players (+1 IR)
            </Typography>
            <Typography variant="body2">
              Salary used/cap: $
              {team.contracts.reduce((acc, contractFragment) => {
                const contract = useFragment(
                  ContractForRosterListFragmentDoc,
                  contractFragment,
                );
                return acc + contract.salary;
              }, 0)}
              /$210
            </Typography>
          </Stack>
        </Stack>

        <Divider />
      </Stack>
    </Card>
  );
};
