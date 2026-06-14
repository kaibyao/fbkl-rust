import { ThemeProvider } from '@mui/material';
import { createTheme } from '@mui/material/styles';
import { FunctionComponent, PropsWithChildren } from 'react';
import {
  cacheExchange,
  Client,
  fetchExchange,
  Provider as GraphQlProvider,
} from 'urql';

const darkTheme = createTheme({
  palette: {
    mode: 'dark',
  },
});

// Same-origin in both dev (vite proxy) and prod (Pages Function proxies /api/* to the Lambda).
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
  <ThemeProvider theme={darkTheme}>
    <GraphQlProvider value={client}>{children}</GraphQlProvider>
  </ThemeProvider>
);
