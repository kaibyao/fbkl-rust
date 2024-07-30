'use server';

import { AppProviders } from '@/app/_components/AppProviders';
import { UserProvider } from '@/app/(authenticated)/_components/UserContext';
import { getUserData } from '@/app/(authenticated)/actions';
import { redirect } from 'next/navigation';

export default async function AuthenticatedLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const userData = await getUserData();

  if (!userData.isLoggedIn) {
    redirect('/login');
  }

  if (!userData.selectedLeagueId) {
    redirect('/leagues');
  }

  return (
    <AppProviders>
      <UserProvider user={userData}>{children}</UserProvider>
    </AppProviders>
  );
}
