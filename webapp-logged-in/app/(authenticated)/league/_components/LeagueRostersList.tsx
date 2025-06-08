import { FunctionComponent } from 'react';
import { LeagueTeamRoster } from '@/app/(authenticated)/league/_components/LeagueTeamRoster';
import Grid from '@mui/material/Grid';

export const LeagueRostersList: FunctionComponent = () => {
  return (
    <Grid container spacing={2}>
      <Grid size={{ xs: 12, md: 6, lg: 4 }}>
        <LeagueTeamRoster />
      </Grid>
      <Grid size={{ xs: 12, md: 6, lg: 4 }}>
        <LeagueTeamRoster />
      </Grid>
      <Grid size={{ xs: 12, md: 6, lg: 4 }}>
        <LeagueTeamRoster />
      </Grid>
      <Grid size={{ xs: 12, md: 6, lg: 4 }}>
        <LeagueTeamRoster />
      </Grid>
    </Grid>
  );
};
