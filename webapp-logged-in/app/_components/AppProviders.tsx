'use client';

import { ApolloClient, ApolloProvider, InMemoryCache } from '@apollo/client';
import { FunctionComponent, PropsWithChildren } from 'react';
import { ThemeProvider } from '@mui/material';
import createTheme from '@mui/material/styles/createTheme';

const darkTheme = createTheme({
  palette: {
    mode: 'dark',
  },
});

const client = new ApolloClient({
  uri: '/api/gql',
  cache: new InMemoryCache(),
});

export const AppProviders: FunctionComponent<PropsWithChildren> = ({
  children,
}) => (
  <ThemeProvider theme={darkTheme}>
    <ApolloProvider client={client}>{children}</ApolloProvider>
  </ThemeProvider>
);
