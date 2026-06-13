import { createFileRoute, Link } from '@tanstack/react-router';
import { Loader2 } from 'lucide-react';
import { useQuery } from 'urql';
import { LeagueListItem } from '@/components/leagues/LeagueListItem';
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
      <h1 className="mb-6 font-heading text-3xl font-black tracking-tight">
        Select a league
      </h1>

      {fetching ? (
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="size-4 animate-spin" />
          Loading leagues...
        </div>
      ) : error ? (
        <p className="text-sm text-destructive">
          An error occurred: {error.message}
        </p>
      ) : data ? (
        data.leagues.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            It looks like you have no leagues.{' '}
            <Link to="/leagues/create" className="text-primary-hot underline">
              Let’s create one
            </Link>
            !
          </p>
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
        <p className="text-sm text-destructive">
          An error occurred... we couldn’t load your leagues. Try again or ask
          Kai to fix this.
        </p>
      )}
    </div>
  );
}
