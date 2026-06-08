import { createFileRoute, Outlet, redirect } from '@tanstack/react-router';
import { UserProvider } from '@/components/UserContext';
import { getUserData } from '@/lib/auth';

export const Route = createFileRoute('/_auth')({
  beforeLoad: async () => {
    const userData = await getUserData();
    if (!userData.isLoggedIn) {
      throw redirect({ to: '/login' });
    }
    return { userData };
  },
  component: AuthLayout,
});

function AuthLayout() {
  const { userData } = Route.useRouteContext();
  return (
    <UserProvider user={userData}>
      <Outlet />
    </UserProvider>
  );
}
