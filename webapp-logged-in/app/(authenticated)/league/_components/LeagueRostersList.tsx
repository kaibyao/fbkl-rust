'use client';

import { FunctionComponent } from 'react';
import { graphql, useFragment } from '@/generated';
import Grid from '@mui/material/Grid';
import { useQuery } from 'urql';
import { TeamForRosterListFragmentDoc } from '@/generated/graphql';
import { LeagueTeamRoster } from '@/app/(authenticated)/league/_components/LeagueTeamRoster';

const getLeagueRosterListQuery = graphql(`
  query GetLeagueRosterList {
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
  }
`);

export const LeagueRostersList: FunctionComponent = () => {
  const [{ data, error, fetching }] = useQuery({
    query: getLeagueRosterListQuery,
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
      {teams.map((teamFragment) => {
        const team = useFragment(TeamForRosterListFragmentDoc, teamFragment);
        return (
          <Grid size={{ xs: 12, md: 6, lg: 4 }} key={team.id}>
            <LeagueTeamRoster team={team} />
          </Grid>
        );
      })}
    </Grid>
  );
};
