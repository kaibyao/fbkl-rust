import { useNavigate } from '@tanstack/react-router';
import { Loader2 } from 'lucide-react';
import { useMutation } from 'urql';
import { Card, CardContent } from '@/components/ui/card';
import { graphql } from '@/generated';
import { LeagueListFragment } from '@/generated/graphql';

const selectLeagueMutation = graphql(`
  mutation SelectLeague($leagueId: Int!) {
    selectLeague(leagueId: $leagueId) {
      id
      name
    }
  }
`);

interface Props {
  league: LeagueListFragment;
}

export const LeagueListItem: React.FC<Props> = ({ league }) => {
  const navigate = useNavigate();
  const [{ fetching, error }, executeSelectLeagueMutation] =
    useMutation(selectLeagueMutation);

  const handleSelectLeague = async () => {
    try {
      await executeSelectLeagueMutation({ leagueId: league.id });
      await navigate({ to: '/league' });
    } catch (e) {
      console.error(e);
      if (error) {
        console.error(error);
      }
    }
  };

  return (
    <Card
      onClick={handleSelectLeague}
      className="cursor-pointer transition-all hover:-translate-y-0.5 hover:ring-primary-hot/60"
    >
      <CardContent className="flex flex-col gap-1">
        <h2 className="font-heading text-base font-bold">
          {league.name}
          <span className="text-muted-foreground">
            {' '}
            — {league.currentTeamUser?.team?.name}
          </span>
        </h2>
        <div className="flex items-end gap-1.5 text-xs text-muted-foreground">
          <span className="text-foreground">
            {league.currentTeamUser?.nickname}
          </span>
          <span>({league.currentTeamUser?.leagueRole})</span>
          {fetching ? <Loader2 className="size-3 animate-spin" /> : null}
        </div>
      </CardContent>
    </Card>
  );
};
