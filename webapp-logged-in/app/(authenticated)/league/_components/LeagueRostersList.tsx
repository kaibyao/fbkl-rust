'use client';

import { FunctionComponent, useMemo } from 'react';
import { graphql } from '@/generated';
import Grid from '@mui/material/Grid';
import { useQuery } from 'urql';
import { LeagueTeamRoster } from '@/app/(authenticated)/league/_components/LeagueTeamRoster';

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
    <Grid container spacing={2}>
      {teams
        .sort((a, b) => a.name.localeCompare(b.name))
        .map((team) => {
          return (
            <Grid size={{ xs: 12, md: 6, lg: 4 }} key={team.id}>
              <LeagueTeamRoster team={team} />
            </Grid>
          );
        })}
    </Grid>
  );
};
