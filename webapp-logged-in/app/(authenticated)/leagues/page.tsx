'use client';

import { LeagueListItem } from '@/app/(authenticated)/leagues/_components/LeagueListItem';
import { gql } from '@apollo/client';
import { useGetUserLeaguesQuery } from '@/generated/graphql';
import CircularProgress from '@mui/material/CircularProgress';
import Grid2 from '@mui/material/Unstable_Grid2';
import Icon from '@mui/material/Icon';
import Link from 'next/link';
import Stack from '@mui/material/Stack';
import Typography from '@mui/material/Typography';

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

  query GetUserLeagues {
    leagues {
      id
      ...LeagueList
    }
  }
`;

export default function LeaguesPage({
  children,
}: {
  children?: React.ReactNode;
}) {
  const {
    error,
    loading,
    data: leagues,
  } = useGetUserLeaguesQuery({
    fetchPolicy: 'network-only',
  });

  return (
    <Stack spacing={3}>
      <Typography variant="h1">Select a league</Typography>
      {loading ? (
        <Stack direction="row" spacing={1}>
          <Typography variant="body2">Loading leagues...</Typography>
          <Icon>
            <CircularProgress />
          </Icon>
        </Stack>
      ) : error ? (
        <Typography variant="body2" color="error">
          An error occurred: {error.message}
        </Typography>
      ) : leagues?.leagues ? (
        <Grid2 container spacing={2}>
          {leagues.leagues.length === 0 ? (
            <Typography variant="body2">
              It looks like you have no leagues.{' '}
              <Link href="/leagues/create">Let’s create one</Link>!
            </Typography>
          ) : (
            leagues.leagues.map((league) => (
              <Grid2 key={league.id} xs={12} sm={6} md={4} lg={3} xl={2}>
                <LeagueListItem key={league.id} league={league} />
              </Grid2>
            ))
          )}
        </Grid2>
      ) : (
        <Typography variant="body2" color="error">
          An error occurred... we couldn’t load your leagues. Try again or ask
          Kai to fix this.
        </Typography>
      )}
      {children}
    </Stack>
  );
}
