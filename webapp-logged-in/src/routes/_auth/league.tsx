import Box from '@mui/material/Box';
import Toolbar from '@mui/material/Toolbar';
import { createFileRoute, Outlet, redirect } from '@tanstack/react-router';
import { LeagueHeader } from '@/components/league/LeagueHeader';
import { LEAGUE_MENU_WIDTH, LeagueMenu } from '@/components/league/LeagueMenu';
import { UserDataProvider } from '@/components/league/UserDataProvider';

export const Route = createFileRoute('/_auth/league')({
  beforeLoad: ({ context }) => {
    if (!context.userData.selectedLeagueId) {
      throw redirect({ to: '/leagues' });
    }
  },
  component: LeagueLayout,
});

function LeagueLayout() {
  const { userData } = Route.useRouteContext();
  return (
    <UserDataProvider userData={userData}>
      <LeagueHeader />

      <LeagueMenu />

      <Box marginLeft={`${LEAGUE_MENU_WIDTH}px`} paddingTop={2}>
        <Toolbar />
        <Box paddingLeft={3} paddingRight={3}>
          <Outlet />
        </Box>
      </Box>
    </UserDataProvider>
  );
}
