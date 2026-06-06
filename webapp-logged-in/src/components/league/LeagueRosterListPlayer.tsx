import PersonIcon from '@mui/icons-material/Person';
import Avatar from '@mui/material/Avatar';
import Chip from '@mui/material/Chip';
import ListItem from '@mui/material/ListItem';
import ListItemAvatar from '@mui/material/ListItemAvatar';
import ListItemText from '@mui/material/ListItemText';
import Tooltip from '@mui/material/Tooltip';
import Typography from '@mui/material/Typography';
import { FunctionComponent } from 'react';
import {
  ContractForRosterListFragment,
  ContractKind,
} from '@/generated/graphql';

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
    photoUrl = contract.leagueOrRealPlayer.realPlayer?.thumbnailUrl;
    position = contract.leagueOrRealPlayer.realPlayer?.position;
    realTeamName = contract.leagueOrRealPlayer.realPlayer?.realTeamName;
  } else if (contract.leagueOrRealPlayer.__typename === 'RealPlayer') {
    photoUrl = contract.leagueOrRealPlayer.thumbnailUrl;
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
    case 'FREE_AGENT':
      return 'FA';
    case 'RESTRICTED_FREE_AGENT':
      return 'RFA';
    case 'ROOKIE':
    case 'ROOKIE_EXTENSION':
      return 'R';
    case 'ROOKIE_DEVELOPMENT':
      return 'RD';
    case 'ROOKIE_DEVELOPMENT_INTERNATIONAL':
      return 'RDI';
    case 'VETERAN':
      return 'V';
    case 'UNRESTRICTED_FREE_AGENT_ORIGINAL_TEAM':
      return 'UFA-20%';
    case 'UNRESTRICTED_FREE_AGENT_VETERAN':
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
