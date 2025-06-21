'use server';

import {
  LEAGUE_MENU_WIDTH,
  LeagueMenu,
} from '@/app/(authenticated)/league/_components/LeagueMenu';
import { LeagueHeader } from '@/app/(authenticated)/league/_components/LeagueHeader';
import { getUserData } from '@/app/(authenticated)/actions';
import { redirect } from 'next/navigation';
import Box from '@mui/material/Box';
import Toolbar from '@mui/material/Toolbar';
import { UserDataProvider } from '@/app/(authenticated)/league/_components/UserDataProvider';

export default async function LeagueLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const userData = await getUserData();

  if (!userData.isLoggedIn) {
    console.log('User not logged in, redirecting to login');
    redirect('/login');
    return null;
  }

  if (!userData.selectedLeagueId) {
    console.log('User has not selected a league, redirecting to leagues');
    redirect('/leagues');
    return null;
  }

  return (
    <UserDataProvider userData={userData}>
      <LeagueHeader />

      <LeagueMenu />

      <Box marginLeft={`${LEAGUE_MENU_WIDTH}px`} paddingTop={2}>
        <Toolbar />
        <Box paddingLeft={3} paddingRight={3}>
          {children}
        </Box>
      </Box>
    </UserDataProvider>
  );
}
