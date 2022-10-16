import { AppBar, Box, Container, Toolbar, Typography } from "@mui/material";
import { FunctionComponent } from "react";
import {
  LEAGUE_MENU_WIDTH,
  LeagueMenu,
} from "@logged-in/src/routes/league/LeagueMenu";
import { Outlet } from "react-router-dom";
import { gql } from "@apollo/client";
import { useGetLeagueQuery } from "@logged-in/generated/graphql";

gql`
  query GetLeague {
    league {
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
  const { data, error, loading } = useGetLeagueQuery();

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
