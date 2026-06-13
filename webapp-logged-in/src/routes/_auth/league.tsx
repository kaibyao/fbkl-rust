import { createFileRoute, Outlet, redirect } from '@tanstack/react-router';
import { LeagueHeader } from '@/components/league/LeagueHeader';
import { LeagueMenu } from '@/components/league/LeagueMenu';
import { UserDataProvider } from '@/components/league/UserDataProvider';
import { SidebarInset, SidebarProvider } from '@/components/ui/sidebar';

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
      <SidebarProvider>
        <LeagueMenu />
        <SidebarInset>
          <LeagueHeader />
          <div className="p-4">
            <Outlet />
          </div>
        </SidebarInset>
      </SidebarProvider>
    </UserDataProvider>
  );
}
