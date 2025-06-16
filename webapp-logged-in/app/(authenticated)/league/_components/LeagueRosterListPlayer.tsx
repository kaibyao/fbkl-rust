import {
  ContractForRosterListFragment,
  ContractKind,
} from '@/generated/graphql';
import { FunctionComponent } from 'react';
import Tooltip from '@mui/material/Tooltip';
import ListItem from '@mui/material/ListItem';
import ListItemText from '@mui/material/ListItemText';
import ListItemAvatar from '@mui/material/ListItemAvatar';
import Avatar from '@mui/material/Avatar';
import PersonIcon from '@mui/icons-material/Person';
import Typography from '@mui/material/Typography';
import Chip from '@mui/material/Chip';

interface Props {
  contract: ContractForRosterListFragment;
  isIr?: boolean;
}

export const LeagueRosterListPlayer: FunctionComponent<Props> = ({
  contract,
  isIr,
}) => {
  let photoUrl = undefined;
  let position = undefined;
  let realTeamName = undefined;

  if (contract.leagueOrRealPlayer.__typename === 'LeaguePlayer') {
    photoUrl = contract.leagueOrRealPlayer.realPlayer?.photoUrl;
    position = contract.leagueOrRealPlayer.realPlayer?.position;
    realTeamName = contract.leagueOrRealPlayer.realPlayer?.realTeamName;
  } else if (contract.leagueOrRealPlayer.__typename === 'RealPlayer') {
    photoUrl = contract.leagueOrRealPlayer.photoUrl;
    position = contract.leagueOrRealPlayer.position;
    realTeamName = contract.leagueOrRealPlayer.realTeamName;
  }

  const positionTeamNameString = generatePositionTeamNameString({
    position,
    realTeamName,
  });

  return (
    <ListItem
      key={contract.id}
      disableGutters
      secondaryAction={
        isIr ? <Chip label="IR" color="warning" size="small" /> : undefined
      }
    >
      <ListItemAvatar>
        {photoUrl ? (
          <Avatar src={photoUrl} />
        ) : (
          <Avatar>
            <PersonIcon />
          </Avatar>
        )}
      </ListItemAvatar>
      <ListItemText
        primary={
          <Typography variant="body2">
            {contract.leagueOrRealPlayer.name}{' '}
            {positionTeamNameString ? `(${positionTeamNameString})` : ''}
          </Typography>
        }
        secondary={
          <Typography variant="body2">
            ${contract.salary} / {contract.yearNumber} /{' '}
            <Tooltip
              arrow
              title={<Typography variant="body2">{contract.kind}</Typography>}
            >
              <span>{abbreviateContractKind(contract.kind)}</span>
            </Tooltip>
          </Typography>
        }
      />
    </ListItem>
  );
};

function abbreviateContractKind(kind: ContractKind): string {
  switch (kind) {
    case ContractKind.FreeAgent:
      return 'FA';
    case ContractKind.RestrictedFreeAgent:
      return 'RFA';
    case ContractKind.Rookie:
    case ContractKind.RookieExtension:
      return 'R';
    case ContractKind.RookieDevelopment:
      return 'RD';
    case ContractKind.RookieDevelopmentInternational:
      return 'RDI';
    case ContractKind.Veteran:
      return 'V';
    case ContractKind.UnrestrictedFreeAgentOriginalTeam:
      return 'UFA-20%';
    case ContractKind.UnrestrictedFreeAgentVeteran:
      return 'UFA-10%';
    default:
      return 'Unknown';
  }
}

function generatePositionTeamNameString({
  position,
  realTeamName,
}: {
  position?: string;
  realTeamName?: string;
}): string {
  if (position && realTeamName) {
    return `${position} – ${realTeamName}`;
  }
  return position || realTeamName || '';
}
