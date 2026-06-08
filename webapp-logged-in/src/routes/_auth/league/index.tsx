import { createFileRoute } from '@tanstack/react-router';
import { LeagueRostersList } from '@/components/league/LeagueRostersList';

export const Route = createFileRoute('/_auth/league/')({
  component: LeagueRostersList,
});
