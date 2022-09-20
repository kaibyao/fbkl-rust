import { Box, Container, Typography } from "@mui/material";
import { FunctionComponent } from "react";

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
