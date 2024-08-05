import { FunctionComponent } from 'react';
import Card from '@mui/material/Card';
import Divider from '@mui/material/Divider';
import Stack from '@mui/material/Stack';
import Typography from '@mui/material/Typography';

export const LeagueTeamRoster: FunctionComponent = () => {
  return (
    <Card
      sx={{
        padding: 2,
      }}
    >
      <Stack spacing={2}>
        <Stack direction="row" spacing={2} justifyContent="space-between">
          <Typography variant="h4">Kai</Typography>
          <Stack>
            <Typography variant="body2">
              Roster size: 22 players (+1 IR)
            </Typography>
            <Typography variant="body2">Salary used/cap: $198/210</Typography>
          </Stack>
        </Stack>

        <Divider />
      </Stack>
    </Card>
  );
};
