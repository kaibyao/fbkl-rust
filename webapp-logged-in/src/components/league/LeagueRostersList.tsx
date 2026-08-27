import { Link } from '@tanstack/react-router';
import { FunctionComponent, useMemo } from 'react';
import { useQuery } from 'urql';
import { LeagueTeamRoster } from '@/components/league/LeagueTeamRoster';
import { ExplainerNote } from '@/components/ui/explainer-note';
import { Stack, StackGap } from '@/components/ui/stack';
import { graphql } from '@/generated';
import { LeagueRulesSection } from '@/lib/league-rules';

const getLeagueRosterListQuery = graphql(`
  query GetLeagueRosterList($datetimeStr: String!) {
    league {
      teams {
        ...TeamForRosterList
      }
    }
  }

  fragment TeamForRosterList on Team {
    id
    name
    contracts {
      ...ContractForRosterList
    }
    salaryCap(datetimeStr: $datetimeStr) {
      salaryCap
      salaryUsed
    }
  }

  fragment ContractForRosterList on Contract {
    id
    yearNumber
    kind
    isIr
    salary
    endOfSeasonYear
    status
    leaguePlayerId
    playerId
    leagueOrRealPlayer {
      __typename
      ... on LeaguePlayer {
        id
        name
        realPlayerId
        isRdiEligible
        realPlayer {
          ...RealPlayerForRosterList
        }
      }
      ... on RealPlayer {
        ...RealPlayerForRosterList
      }
    }
  }

  fragment RealPlayerForRosterList on RealPlayer {
    id
    name
    position
    thumbnailUrl
    realTeamName
  }
`);

export const LeagueRostersList: FunctionComponent = () => {
  const datetimeStr = useMemo(() => new Date().toISOString(), []);
  const [{ data, error, fetching }] = useQuery({
    query: getLeagueRosterListQuery,
    variables: {
      datetimeStr,
    },
  });

  if (fetching) {
    return <div>Loading...</div>;
  }
  if (error) {
    return <div>Error: {error.message}</div>;
  }

  const teams = data?.league?.teams;

  if (!teams) {
    return <div>No teams found</div>;
  }

  return (
    <Stack gap={StackGap.Md}>
      <ExplainerNote
        action={
          <Link
            to="/league/rules"
            hash={LeagueRulesSection.SalaryCap}
            className="shrink-0 text-sm font-bold underline-offset-4 hover:underline"
          >
            Learn more
          </Link>
        }
      >
        Every salary here counts against your cap except the one in the IR slot.
        Drop a player mid-season and your cap falls by 20% of his salary,
        rounded up, for the rest of the season.
      </ExplainerNote>

      <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
        {teams
          .sort((a, b) => a.name.localeCompare(b.name))
          .map((team) => (
            <LeagueTeamRoster key={team.id} team={team} />
          ))}
      </div>
    </Stack>
  );
};
