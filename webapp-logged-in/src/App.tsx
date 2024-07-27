import { ApolloClient, ApolloProvider, InMemoryCache } from '@apollo/client';
import { AppRoutes } from '@/src/AppRoutes';
import { FunctionComponent } from 'react';
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

export const App: FunctionComponent = () => (
  <ThemeProvider theme={darkTheme}>
    <ApolloProvider client={client}>
      <AppRoutes />
    </ApolloProvider>
  </ThemeProvider>
);
