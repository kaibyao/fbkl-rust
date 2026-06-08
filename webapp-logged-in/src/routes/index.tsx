import { createFileRoute, redirect } from '@tanstack/react-router';
import { getUserData } from '@/lib/auth';

export const Route = createFileRoute('/')({
  beforeLoad: async () => {
    const userData = await getUserData();
    if (!userData.isLoggedIn) {
      throw redirect({ to: '/login' });
    }
    if (!userData.selectedLeagueId) {
      throw redirect({ to: '/leagues' });
    }
    throw redirect({ to: '/league' });
  },
});
