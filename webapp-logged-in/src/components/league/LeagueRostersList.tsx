import { FunctionComponent, useMemo } from 'react';
import { useQuery } from 'urql';
import { LeagueTeamRoster } from '@/components/league/LeagueTeamRoster';
import { graphql } from '@/generated';

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
    <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
      {teams
        .sort((a, b) => a.name.localeCompare(b.name))
        .map((team) => (
          <LeagueTeamRoster key={team.id} team={team} />
        ))}
    </div>
  );
};
