import { FunctionComponent } from 'react';
import Box from "@mui/material/Box";
import Container from "@mui/material/Container";
import Typography from "@mui/material/Typography";

export const NotFound404: FunctionComponent = () => (
  <Container>
    <Box mt={3}>
      <Typography variant="h2">Not found</Typography>
      <Typography variant="body1">
        We could not find the page you are looking for.
      </Typography>
    </Box>
  </Container>
);
