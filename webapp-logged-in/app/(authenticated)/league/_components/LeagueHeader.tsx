'use client';

import { graphql } from '@/generated';
import { GetLeagueForHeaderQuery } from '@/generated/graphql';
import AppBar from '@mui/material/AppBar';
import Toolbar from '@mui/material/Toolbar';
import Typography from '@mui/material/Typography';
import { useQuery } from 'urql';

export const getLeagueForHeaderQuery = graphql(`
  query GetLeagueForHeader {
    league {
      id
      name
    }
  }
`);

export const LeagueHeader: React.FC = () => {
  const [{ data, error, fetching }] = useQuery<GetLeagueForHeaderQuery>({
    query: getLeagueForHeaderQuery,
  });

  return (
    <AppBar
      position="fixed"
      sx={(theme) => ({
        zIndex: theme.zIndex.drawer + 1,
      })}
    >
      <Toolbar>
        <Typography variant="h6" noWrap component="div">
          {fetching
            ? 'Loading league...'
            : error
              ? 'Error occurred'
              : data?.league?.name}
        </Typography>
      </Toolbar>
    </AppBar>
  );
};
