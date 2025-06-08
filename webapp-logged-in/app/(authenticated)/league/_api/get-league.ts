import { graphql } from '@/generated';

export const getLeagueQuery = graphql(`
  query GetLeague {
    league {
      id
      name
    }
  }
`);
