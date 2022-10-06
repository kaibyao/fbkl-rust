import { FunctionComponent } from "react";
import {
  // Box,
  // Card,
  // CardActionArea,
  // CardContent,
  Grid,
  // Typography,
} from "@mui/material";
// import { LeagueListFragment } from "@logged-in/generated/graphql";
import { gql } from "@apollo/client";
// import { useNavigate } from "react-router-dom";

export const LEAGUE_LIST_FRAGMENT = gql`
  fragment LeagueList on League {
    id
    name
    teams {
      id
    }
  }
`;

// interface Props {
//   leagues: LeagueListFragment[];
// }

export const LeagueList: FunctionComponent /*<Props>*/ = (/*{ leagues }*/) => {
  // const navigate = useNavigate();

  return (
    <Grid container spacing={2}>
      {/* {leagues.map((league) => (
        <Grid key={league.id} item xs={12} sm={6} md={4} lg={3} xl={2}>
          <Card variant="outlined">
            <CardActionArea onClick={() => navigate(`/leagues/${league.id}`)}>
              <CardContent>
                <Typography variant="h4" color="ButtonFace">
                  {league.name}
                </Typography>
                <Box sx={{ display: "flex", alignItems: "flex-end" }}>
                  <Typography variant="body1" sx={{ mr: 1 }}>
                    {league.userNickname}
                  </Typography>
                  <Typography variant="body2" color="GrayText">
                    ({league.userRole.toLowerCase()})
                  </Typography>
                </Box>
              </CardContent>
            </CardActionArea>
          </Card>
        </Grid>
      ))} */}
    </Grid>
  );
};
