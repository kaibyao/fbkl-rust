'use server';

import { redirect } from 'next/navigation';
import { ReactNode } from 'react';
import { AppProviders } from '@/app/_components/AppProviders';
import { UserProvider } from '@/app/(authenticated)/_components/UserContext';
import { getUserData } from '@/app/(authenticated)/actions';

export default async function AuthenticatedLayout({
  children,
}: {
  children: ReactNode;
}) {
  const userData = await getUserData();

  if (!userData.isLoggedIn) {
    console.log('User not logged in, redirecting to login');
    redirect('/login');
  }

  return (
    <AppProviders>
      <UserProvider user={userData}>{children}</UserProvider>
    </AppProviders>
  );
}
