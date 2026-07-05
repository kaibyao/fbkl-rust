import { createFileRoute, Link } from '@tanstack/react-router';
import { Loader2 } from 'lucide-react';
import { useQuery } from 'urql';
import { LeagueListItem } from '@/components/leagues/LeagueListItem';
import {
  Stack,
  StackAlign,
  StackDirection,
  StackGap,
} from '@/components/ui/stack';
import { Typography, TypographyVariant } from '@/components/ui/typography';
import { graphql } from '@/generated';
import { GetUserLeaguesQuery, LeagueListFragment } from '@/generated/graphql';

export const Route = createFileRoute('/_auth/leagues/')({
  component: LeaguesPage,
});

const getUserLeaguesQuery = graphql(`
  fragment LeagueList on League {
    id
    name
    currentTeamUser {
      leagueRole
      nickname
      team {
        id
        name
      }
    }
  }

  query GetUserLeagues {
    leagues {
      id
      ...LeagueList
    }
  }
`);

function LeaguesPage() {
  const [{ error, fetching, data }] = useQuery<GetUserLeaguesQuery>({
    query: getUserLeaguesQuery,
  });

  return (
    <div className="mx-auto w-full max-w-6xl px-6 py-10">
      <Typography variant={TypographyVariant.Heading1} className="mb-6">
        Select a league
      </Typography>

      {fetching ? (
        <Stack
          direction={StackDirection.Row}
          align={StackAlign.Center}
          gap={StackGap.Sm}
          className="text-sm text-muted-foreground"
        >
          <Loader2 className="size-4 animate-spin" />
          Loading leagues...
        </Stack>
      ) : error ? (
        <Typography variant={TypographyVariant.Error}>
          An error occurred: {error.message}
        </Typography>
      ) : data ? (
        data.leagues.length === 0 ? (
          <Typography variant={TypographyVariant.Muted}>
            It looks like you have no leagues.{' '}
            <Link to="/leagues/create" className="text-primary-hot underline">
              Let’s create one
            </Link>
            !
          </Typography>
        ) : (
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4">
            {data.leagues.map((league) => (
              <LeagueListItem
                key={league.id}
                league={league as LeagueListFragment}
              />
            ))}
          </div>
        )
      ) : (
        <Typography variant={TypographyVariant.Error}>
          An error occurred... we couldn’t load your leagues. Try again or ask
          Kai to fix this.
        </Typography>
      )}
    </div>
  );
}
