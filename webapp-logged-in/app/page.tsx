import { getUserData } from '@/app/(authenticated)/actions';
import { redirect } from 'next/navigation';

export default async function RootPage() {
  const userData = await getUserData();

  if (!userData.isLoggedIn) {
    redirect('/login');
  }

  if (!userData.selectedLeagueId) {
    redirect('/leagues');
  }

  redirect('/league');
}
