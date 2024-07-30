'use client';

import { useGetLeagueQuery } from '@/generated/graphql';
import Toolbar from '@mui/material/Toolbar';
import Typography from '@mui/material/Typography';

export const LeagueHeader: React.FC = () => {
  const { data, error, loading } = useGetLeagueQuery();
  return (
    <Toolbar>
      <Typography variant="h6" noWrap component="div">
        {loading
          ? 'Loading league...'
          : error
            ? 'Error occurred'
            : data?.league?.name}
      </Typography>
    </Toolbar>
  );
};
