import { User } from 'lucide-react';
import { FunctionComponent } from 'react';
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar';
import { Badge } from '@/components/ui/badge';
import {
  Stack,
  StackAlign,
  StackDirection,
  StackGap,
} from '@/components/ui/stack';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { Typography, TypographyVariant } from '@/components/ui/typography';
import { ContractKind } from '@/generated/enums';
import { ContractForRosterListFragment } from '@/generated/graphql';

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
    <Stack
      render={<li />}
      direction={StackDirection.Row}
      align={StackAlign.Center}
      gap={StackGap.Sm}
      className="py-1.5"
    >
      <Avatar size="sm">
        {photoUrl ? <AvatarImage src={photoUrl} alt="" /> : null}
        <AvatarFallback>
          <User className="size-3.5" />
        </AvatarFallback>
      </Avatar>

      <div className="min-w-0 flex-1">
        <Typography variant={TypographyVariant.BodyStrong} className="truncate">
          {contract.leagueOrRealPlayer.name}{' '}
          {positionTeamNameString && (
            <Typography
              variant={TypographyVariant.InlineMuted}
              render={<span />}
            >
              ({positionTeamNameString})
            </Typography>
          )}
        </Typography>
        <Typography
          variant={TypographyVariant.MutedSm}
          className="tabular-nums"
        >
          {contract.salary == null ? 'TBD' : `$${contract.salary}`} /{' '}
          {contract.yearNumber} /{' '}
          <Tooltip>
            <TooltipTrigger
              render={
                <span className="cursor-default underline decoration-dotted" />
              }
            >
              {abbreviateContractKind(contract.kind)}
            </TooltipTrigger>
            <TooltipContent>{contract.kind}</TooltipContent>
          </Tooltip>
        </Typography>
      </div>

      {isIr && (
        <Badge variant="outline" className="border-amber-500/40 text-amber-500">
          IR
        </Badge>
      )}
    </Stack>
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
