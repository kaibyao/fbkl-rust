'use client';

import {
  LEAGUE_MENU_WIDTH,
  LeagueMenu,
} from '@/app/(authenticated)/league/_components/LeagueMenu';
import { LeagueHeader } from '@/app/(authenticated)/league/_components/LeagueHeader';
import AppBar from '@mui/material/AppBar';
import Box from '@mui/material/Box';
import Toolbar from '@mui/material/Toolbar';

export default function LeagueLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <>
      <AppBar
        position="fixed"
        sx={{ zIndex: (theme) => theme.zIndex.drawer + 1 }}
      >
        <LeagueHeader />
      </AppBar>

      <LeagueMenu />

      <Box marginLeft={`${LEAGUE_MENU_WIDTH}px`} paddingTop={2}>
        <Toolbar />
        <Box paddingLeft={3} paddingRight={3}>
          {children}
        </Box>
      </Box>
    </>
  );
}
