import { useQuery } from 'urql';
import { Separator } from '@/components/ui/separator';
import { SidebarTrigger } from '@/components/ui/sidebar';
import { graphql } from '@/generated';
import { GetLeagueForHeaderQuery } from '@/generated/graphql';

export const getLeagueForHeaderQuery = graphql(`
  query GetLeagueForHeader {
    league {
      id
      name
    }
  }
`);

export const LeagueHeader: React.FC = () => {
  const [{ data, error, fetching }] = useQuery<GetLeagueForHeaderQuery>({
    query: getLeagueForHeaderQuery,
  });

  return (
    <header className="sticky top-0 z-10 flex h-12 shrink-0 items-center gap-2 border-b bg-background/80 px-4 backdrop-blur">
      <SidebarTrigger className="-ml-1" />
      <Separator orientation="vertical" className="mr-1 h-4" />
      <h1 className="truncate font-heading text-base font-bold">
        {fetching
          ? 'Loading league...'
          : error
            ? 'Error occurred'
            : data?.league?.name}
      </h1>
    </header>
  );
};
