import { FunctionComponent, PropsWithChildren } from 'react';
import {
  cacheExchange,
  Client,
  fetchExchange,
  Provider as GraphQlProvider,
} from 'urql';
import { TooltipProvider } from '@/components/ui/tooltip';

const client = new Client({
  url: '/api/gql',
  exchanges: [cacheExchange, fetchExchange],
  fetchOptions: {
    credentials: 'include',
  },
});

export const AppProviders: FunctionComponent<PropsWithChildren> = ({
  children,
}) => (
  <GraphQlProvider value={client}>
    <TooltipProvider>{children}</TooltipProvider>
  </GraphQlProvider>
);
