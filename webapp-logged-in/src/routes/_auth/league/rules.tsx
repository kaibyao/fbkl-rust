import { createFileRoute } from '@tanstack/react-router';
import { LeagueRules } from '@/components/league/LeagueRules';

export const Route = createFileRoute('/_auth/league/rules')({
  component: LeagueRules,
});
