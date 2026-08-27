import { FunctionComponent } from 'react';
import { Badge } from '@/components/ui/badge';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { ContractForRosterListFragment } from '@/generated/graphql';
import { CONTRACT_KIND_DISPLAY } from '@/lib/contract.utils';

type Props = Pick<ContractForRosterListFragment, 'kind' | 'yearNumber'>;

/** A contract's kind and year as one zinc chip, e.g. "V Y2/3" or "RFA". Hover spells the kind out. */
export const ContractChip: FunctionComponent<Props> = ({
  kind,
  yearNumber,
}) => {
  const { abbreviation, label, finalYearNumber } = CONTRACT_KIND_DISPLAY[kind];

  return (
    <Tooltip>
      <TooltipTrigger
        render={<Badge variant="secondary" className="cursor-default" />}
      >
        {finalYearNumber === null
          ? abbreviation
          : `${abbreviation} Y${yearNumber}/${finalYearNumber}`}
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
};
