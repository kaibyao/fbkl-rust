import {
  Box,
  Card,
  CardActionArea,
  CardContent,
  Grid,
  Typography,
} from "@mui/material";
import { FunctionComponent } from "react";
import { LeagueListFragment } from "@logged-in/generated/graphql";
import { gql } from "@apollo/client";
import { selectLeague } from "@logged-in/src/api/select-league";
import { useAsyncRequest } from "@logged-in/src/api/api-hook";
import { useNavigate } from "react-router-dom";

gql`
  fragment LeagueList on League {
    id
    name
    currentTeamUser {
      leagueRole
      nickname
      team {
        id
        name
      }
    }
  }
`;

interface Props {
  leagues: LeagueListFragment[];
}

export const LeagueList: FunctionComponent<Props> = ({ leagues }) => {
  const navigate = useNavigate();
  const [wrappedSelectLeague, { error, loading }] =
    useAsyncRequest(selectLeague);

  const handleLeagueSelect = async (leagueId: number) => {
    try {
      await wrappedSelectLeague(leagueId);
      navigate(`/app/league`);
    } catch (e) {
      console.error(e);
    }
  };

  return (
    <Grid container spacing={2}>
      {error ? (
        <Typography variant="body2">
          An error occurred: {String(error)}
        </Typography>
      ) : null}

      {leagues.map((league) => (
        <Grid key={league.id} item xs={12} sm={6} md={4} lg={3} xl={2}>
          <Card variant="outlined">
            <CardActionArea onClick={() => handleLeagueSelect(league.id)}>
              <CardContent>
                <Typography variant="h4" color="ButtonFace">
                  {league.name} - {league.currentTeamUser?.team?.name}
                </Typography>
                <Box sx={{ display: "flex", alignItems: "flex-end" }}>
                  <Typography variant="body1" sx={{ mr: 1 }}>
                    {league.currentTeamUser?.nickname}
                  </Typography>
                  <Typography variant="body2" color="GrayText">
                    ({league.currentTeamUser?.leagueRole})
                  </Typography>
                  {loading ? (
                    <Typography variant="body2" color="GrayText">
                      Loading...
                    </Typography>
                  ) : null}
                </Box>
              </CardContent>
            </CardActionArea>
          </Card>
        </Grid>
      ))}
    </Grid>
  );
};
