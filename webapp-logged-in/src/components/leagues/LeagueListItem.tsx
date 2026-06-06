import Box from '@mui/material/Box';
import Card from '@mui/material/Card';
import CardActionArea from '@mui/material/CardActionArea';
import CardContent from '@mui/material/CardContent';
import Typography from '@mui/material/Typography';
import { useNavigate } from '@tanstack/react-router';
import { useMutation } from 'urql';
import { graphql } from '@/generated';
import { LeagueListFragment } from '@/generated/graphql';

const selectLeagueMutation = graphql(`
  mutation SelectLeague($leagueId: Int!) {
    selectLeague(leagueId: $leagueId) {
      id
      name
    }
  }
`);

interface Props {
  league: LeagueListFragment;
}

export const LeagueListItem: React.FC<Props> = ({ league }) => {
  const navigate = useNavigate();
  const [{ fetching, error }, executeSelectLeagueMutation] =
    useMutation(selectLeagueMutation);

  const handleSelectLeague = async () => {
    try {
      await executeSelectLeagueMutation({ leagueId: league.id });
      await navigate({ to: '/league' });
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
          <Box sx={{ display: 'flex', alignItems: 'flex-end' }}>
            <Typography variant="body1" sx={{ mr: 1 }}>
              {league.currentTeamUser?.nickname}
            </Typography>
            <Typography variant="body2" color="GrayText">
              ({league.currentTeamUser?.leagueRole})
            </Typography>
            {fetching ? (
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
