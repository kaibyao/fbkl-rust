import { FunctionComponent } from 'react';
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar';
import { Badge } from '@/components/ui/badge';
import {
  Stack,
  StackAlign,
  StackDirection,
  StackGap,
} from '@/components/ui/stack';
import { TermTip } from '@/components/ui/term-tip';
import { Typography, TypographyVariant } from '@/components/ui/typography';
import { ContractForRosterListFragment } from '@/generated/graphql';
import {
  CONTRACT_KIND_DISPLAY,
  isFinalContractYear,
} from '@/lib/contract.utils';
import { getInitials } from '@/lib/name.utils';
import { TERMS } from '@/lib/terms';
import { cn } from '@/lib/utils';

interface Props {
  contract: ContractForRosterListFragment;
}

export const LeagueRosterListPlayer: FunctionComponent<Props> = ({
  contract,
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
  const playerName = contract.leagueOrRealPlayer.name;
  const { abbreviation, label, finalYearNumber } =
    CONTRACT_KIND_DISPLAY[contract.kind];

  return (
    <Stack
      render={<li />}
      direction={StackDirection.Row}
      align={StackAlign.Center}
      gap={StackGap.Md}
      className="py-[9px]"
    >
      <Avatar size="lg" className={cn(contract.isIr && 'opacity-70')}>
        {photoUrl ? <AvatarImage src={photoUrl} alt="" /> : null}
        <AvatarFallback>{getInitials(playerName)}</AvatarFallback>
      </Avatar>

      <div className="min-w-0 flex-1">
        <Typography
          variant={TypographyVariant.BodyStrong}
          className="truncate"
          title={playerName}
        >
          {playerName}
        </Typography>
        {positionTeamNameString && (
          <Typography variant={TypographyVariant.MutedSm} className="truncate">
            {positionTeamNameString}
          </Typography>
        )}
      </div>

      <Stack
        direction={StackDirection.Row}
        align={StackAlign.Center}
        gap={StackGap.Sm}
        className="shrink-0"
      >
        <Typography
          variant={TypographyVariant.Stat}
          // An IR salary is inactive, so display it as secondary.
          className={cn(contract.isIr && 'text-muted-foreground')}
        >
          {contract.salary == null ? (
            <TermTip label={TERMS.TBD}>TBD</TermTip>
          ) : (
            `$${contract.salary}`
          )}
        </Typography>
        <TermTip
          label={label}
          render={<Badge variant="secondary" className="cursor-help" />}
        >
          {finalYearNumber === null
            ? abbreviation
            : `${abbreviation} Y${contract.yearNumber}/${finalYearNumber}`}
        </TermTip>
        {isFinalContractYear(contract) && (
          <Badge variant="outline">Final yr</Badge>
        )}
        {contract.isIr && (
          <TermTip
            label={TERMS.IR}
            render={<Badge variant="secondary" className="cursor-help" />}
          >
            IR
          </TermTip>
        )}
      </Stack>
    </Stack>
  );
};

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
