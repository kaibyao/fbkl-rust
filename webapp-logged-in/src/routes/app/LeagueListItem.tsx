import {
  Box,
  Card,
  CardActionArea,
  CardContent,
  Typography,
} from "@mui/material";
import {
  LeagueListFragment,
  useSelectLeagueMutation,
} from "@logged-in/generated/graphql";
import { gql } from "@apollo/client";
import { useNavigate } from "react-router-dom";

gql`
  mutation SelectLeague($leagueId: Int!) {
    selectLeague(leagueId: $leagueId) {
      id
      name
    }
  }
`;

interface Props {
  league: LeagueListFragment;
}

export const LeagueListItem: React.FC<Props> = ({ league }) => {
  const navigate = useNavigate();
  const [selectLeagueMutation, { loading, error }] = useSelectLeagueMutation();

  const handleSelectLeague = async () => {
    try {
      await selectLeagueMutation({
        variables: {
          leagueId: league.id,
        },
      });
      navigate(`/app/league`);
    } catch (e) {
      console.error(e);
      if (error) {
        console.error(error);
      }
    }
  };

  return (
    <Card variant="outlined">
      <CardActionArea onClick={handleSelectLeague}>
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
  );
};
