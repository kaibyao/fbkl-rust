import "@logged-in/src/App.css";
import { ApolloClient, ApolloProvider, InMemoryCache } from "@apollo/client";
import { AppRoutes } from "@logged-in/src/AppRoutes";
import { FunctionComponent } from "react";
import { ThemeProvider, createTheme } from "@mui/material/styles";

const darkTheme = createTheme({
  palette: {
    mode: "dark",
  },
});

const client = new ApolloClient({
  uri: "/api/graphql",
  cache: new InMemoryCache(),
});

export const App: FunctionComponent = () => (
  <ThemeProvider theme={darkTheme}>
    <ApolloProvider client={client}>
      <AppRoutes />
    </ApolloProvider>
  </ThemeProvider>
);
