import { Container, Typography } from "@mui/material";
import { FunctionComponent } from "react";
import { LeagueList } from "@logged-in/src/routes/app/LeagueList";
import {
  LeagueListFragmentDoc,
  useGetUserLeaguesQuery,
} from "@logged-in/generated/graphql";
import { Link, Outlet } from "react-router-dom";
import { gql } from "@apollo/client";

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
    fetchPolicy: "network-only",
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
          Looks like you have no leagues.{" "}
          <Link to="/app/create">Create one</Link>.
        </Typography>
      )}

      <Outlet />
    </Container>
  );
};
