import { FunctionComponent } from 'react';
import { LeagueList } from '@/src/routes/app/LeagueList';
import {
  LeagueListFragmentDoc,
  useGetUserLeaguesQuery,
} from '@/generated/graphql';
import { Link, Outlet } from 'react-router-dom';
import { gql } from '@apollo/client';
import Container from "@mui/material/Container";
import Typography from "@mui/material/Typography";

gql`
  ${LeagueListFragmentDoc}

  query GetUserLeagues {
    leagues {
      id
      ...LeagueList
    }
  }
`;

export const UserLeaguesRoute: FunctionComponent = () => {
  const { error, loading, data } = useGetUserLeaguesQuery({
    fetchPolicy: 'network-only',
  });

  if (error) {
    console.error(error);
    return <div>An error happened: {error.message}</div>;
  }

  if (loading) {
    return <div>Loading...</div>;
  }

  return (
    <Container sx={{ mt: 3 }}>
      <Typography variant="h3" sx={{ mb: 3 }}>
        Leagues
      </Typography>
      {data?.leagues.length ? (
        <LeagueList leagues={data.leagues} />
      ) : (
        <Typography variant="body2">
          Looks like you have no leagues.{' '}
          <Link to="/app/create">Create one</Link>.
        </Typography>
      )}

      <Outlet />
    </Container>
  );
};
