import { FunctionComponent, PropsWithChildren } from 'react';
import {
  cacheExchange,
  Client,
  fetchExchange,
  Provider as GraphQlProvider,
} from 'urql';
import { TooltipProvider } from '@/components/ui/tooltip';

// Same-origin in both dev (vite proxy) and prod (Pages Function proxies /api/* to the Lambda).
const client = new Client({
  url: '/api/gql',
  // urql defaults preferGetMethod to "within-url-limit"; our /api/gql GET serves the GraphiQL IDE, so force POST.
  preferGetMethod: false,
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
