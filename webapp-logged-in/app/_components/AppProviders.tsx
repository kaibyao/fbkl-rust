'use client';

import { FunctionComponent, PropsWithChildren } from 'react';
import {
  Client,
  cacheExchange,
  fetchExchange,
  Provider as GraphQlProvider,
} from 'urql';
import { ThemeProvider } from '@mui/material';
import { createTheme } from '@mui/material/styles';

const darkTheme = createTheme({
  palette: {
    mode: 'dark',
  },
});

const client = new Client({
  url: '/api/gql',
  exchanges: [cacheExchange, fetchExchange],
});

export const AppProviders: FunctionComponent<PropsWithChildren> = ({
  children,
}) => (
  <ThemeProvider theme={darkTheme}>
    <GraphQlProvider value={client}>{children}</GraphQlProvider>
  </ThemeProvider>
);
