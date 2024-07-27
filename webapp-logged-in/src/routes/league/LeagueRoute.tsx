import { FunctionComponent } from 'react';
import { LEAGUE_MENU_WIDTH, LeagueMenu } from '@/src/routes/league/LeagueMenu';
import { Outlet } from 'react-router-dom';
import { gql } from '@apollo/client';
import { useGetLeagueQuery } from '@/generated/graphql';
import AppBar from "@mui/material/AppBar";
import Box from "@mui/material/Box";
import Container from "@mui/material/Container";
import Toolbar from "@mui/material/Toolbar";
import Typography from "@mui/material/Typography";

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
              ? 'Loading league...'
              : error
                ? 'Error occurred'
                : data?.league?.name}
          </Typography>
        </Toolbar>
      </AppBar>

      <LeagueMenu />

      <Box marginLeft={`${LEAGUE_MENU_WIDTH}px`} paddingTop={3}>
        <Toolbar />
        <Container>
          <Outlet />
        </Container>
      </Box>
    </>
  );
};
