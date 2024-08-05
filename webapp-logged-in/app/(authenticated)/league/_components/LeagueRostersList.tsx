import { FunctionComponent } from 'react';
import { LeagueTeamRoster } from '@/app/(authenticated)/league/_components/LeagueTeamRoster';
import Grid2 from '@mui/material/Unstable_Grid2/Grid2';

export const LeagueRostersList: FunctionComponent = () => {
  return (
    <Grid2 container spacing={2}>
      <Grid2 xs={12} md={6} lg={4}>
        <LeagueTeamRoster />
      </Grid2>
      <Grid2 xs={12} md={6} lg={4}>
        <LeagueTeamRoster />
      </Grid2>
      <Grid2 xs={12} md={6} lg={4}>
        <LeagueTeamRoster />
      </Grid2>
      <Grid2 xs={12} md={6} lg={4}>
        <LeagueTeamRoster />
      </Grid2>
    </Grid2>
  );
};
