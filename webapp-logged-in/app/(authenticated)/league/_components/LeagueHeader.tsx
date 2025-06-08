'use client';

import { useGetLeagueQuery } from '@/generated/graphql';
import AppBar from '@mui/material/AppBar';
import Toolbar from '@mui/material/Toolbar';
import Typography from '@mui/material/Typography';

export const LeagueHeader: React.FC = () => {
  const { data, error, loading } = useGetLeagueQuery();
  return (
    <AppBar
      position="fixed"
      sx={(theme) => ({
        zIndex: theme.zIndex.drawer + 1,
      })}
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
  );
};
