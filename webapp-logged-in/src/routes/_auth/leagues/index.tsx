import CircularProgress from '@mui/material/CircularProgress';
import Grid from '@mui/material/Grid';
import Icon from '@mui/material/Icon';
import Stack from '@mui/material/Stack';
import Typography from '@mui/material/Typography';
import { createFileRoute, Link } from '@tanstack/react-router';
import { useQuery } from 'urql';
import { LeagueListItem } from '@/components/leagues/LeagueListItem';
import { graphql } from '@/generated';
import { GetUserLeaguesQuery, LeagueListFragment } from '@/generated/graphql';

export const Route = createFileRoute('/_auth/leagues/')({
  component: LeaguesPage,
});

const getUserLeaguesQuery = graphql(`
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
`);

function LeaguesPage() {
  const [{ error, fetching, data }] = useQuery<GetUserLeaguesQuery>({
    query: getUserLeaguesQuery,
  });

  return (
    <Stack spacing={3}>
      <Typography variant="h1">Select a league</Typography>
      {fetching ? (
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
      ) : data ? (
        <Grid container spacing={2}>
          {data.leagues.length === 0 ? (
            <Typography variant="body2">
              It looks like you have no leagues.{' '}
              <Link to="/leagues/create">Let’s create one</Link>!
            </Typography>
          ) : (
            data.leagues.map((league) => (
              <Grid
                key={league.id}
                size={{ xs: 12, sm: 6, md: 4, lg: 3, xl: 2 }}
              >
                <LeagueListItem
                  key={league.id}
                  league={league as LeagueListFragment}
                />
              </Grid>
            ))
          )}
        </Grid>
      ) : (
        <Typography variant="body2" color="error">
          An error occurred... we couldn’t load your leagues. Try again or ask
          Kai to fix this.
        </Typography>
      )}
    </Stack>
  );
}
