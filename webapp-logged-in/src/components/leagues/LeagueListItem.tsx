import { useNavigate } from '@tanstack/react-router';
import { Loader2 } from 'lucide-react';
import { useMutation } from 'urql';
import { Card, CardContent } from '@/components/ui/card';
import {
  Stack,
  StackAlign,
  StackDirection,
  StackGap,
} from '@/components/ui/stack';
import { Typography, TypographyVariant } from '@/components/ui/typography';
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
      <CardContent>
        <Stack gap={StackGap.Xs}>
          <Typography variant={TypographyVariant.Heading2}>
            {league.name}
            <Typography
              variant={TypographyVariant.InlineMuted}
              render={<span />}
            >
              {' '}
              — {league.currentTeamUser?.team?.name}
            </Typography>
          </Typography>
          <Stack
            direction={StackDirection.Row}
            align={StackAlign.End}
            gap={StackGap.Sm}
            className="text-xs text-muted-foreground"
          >
            <span className="text-foreground">
              {league.currentTeamUser?.nickname}
            </span>
            <span>({league.currentTeamUser?.leagueRole})</span>
            {fetching ? <Loader2 className="size-3 animate-spin" /> : null}
          </Stack>
        </Stack>
      </CardContent>
    </Card>
  );
};
