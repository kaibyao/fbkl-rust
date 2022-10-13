import { AppBar, Box, Container, Toolbar, Typography } from "@mui/material";
import { FunctionComponent, useEffect } from "react";
import {
  LEAGUE_MENU_WIDTH,
  LeagueMenu,
} from "@logged-in/src/routes/leagues/LeagueMenu";
import { Outlet, useParams } from "react-router-dom";
import { gql } from "@apollo/client";
import { useGetLeagueLazyQuery } from "@logged-in/generated/graphql";

gql`
  query GetLeague($leagueId: Int!) {
    league(id: $leagueId) {
      id
      ...LeagueRoute
    }
  }

  fragment LeagueRoute on League {
    id
    name
    teams {
      id
      name
      teamUsers {
        leagueRole
        nickname
      }
    }
  }
`;

export const LeagueRoute: FunctionComponent = () => {
  const { leagueId: leagueIdStr } = useParams();
  const [getLeague, { data, error, loading }] = useGetLeagueLazyQuery();

  useEffect(() => {
    if (leagueIdStr) {
      const leagueId = parseInt(leagueIdStr, 10);
      if (!isNaN(leagueId)) {
        getLeague({
          variables: {
            leagueId,
          },
        });
      }
    }
  }, [getLeague, leagueIdStr]);

  return (
    <>
      <AppBar
        position="fixed"
        sx={{ zIndex: (theme) => theme.zIndex.drawer + 1 }}
      >
        <Toolbar>
          <Typography variant="h6" noWrap component="div">
            {loading
              ? "Loading league..."
              : error
              ? "Error occurred"
              : data?.league?.name}
          </Typography>
        </Toolbar>
      </AppBar>

      <LeagueMenu />

      <Box ml={`${LEAGUE_MENU_WIDTH}px`}>
        <Toolbar />
        <Container>
          <Outlet />
        </Container>
      </Box>
    </>
  );
};
