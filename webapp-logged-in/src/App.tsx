import * as createThemeDefault from '@mui/material/styles/createTheme';
import { ApolloClient, ApolloProvider, InMemoryCache } from '@apollo/client';
import { AppRoutes } from '@logged-in/src/AppRoutes';
import { FunctionComponent } from 'react';
import { ThemeProvider } from '@mui/material';

const darkTheme = createThemeDefault.default({
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
