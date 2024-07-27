import { FunctionComponent } from 'react';
import { LeagueListFragment } from '@/generated/graphql';
import { LeagueListItem } from '@/src/routes/app/LeagueListItem';
import { gql } from '@apollo/client';
import Grid from "@mui/material/Grid";

gql`
  fragment LeagueList on League {
    id
    name
    currentTeamUser {
      leagueRole
      nickname
      team {
        id
        name
      }
    }
  }
`;

interface Props {
  leagues: LeagueListFragment[];
}

export const LeagueList: FunctionComponent<Props> = ({ leagues }) => (
  <Grid container spacing={2}>
    {leagues.map((league) => (
      <Grid key={league.id} item xs={12} sm={6} md={4} lg={3} xl={2}>
        <LeagueListItem key={league.id} league={league} />
      </Grid>
    ))}
  </Grid>
);
