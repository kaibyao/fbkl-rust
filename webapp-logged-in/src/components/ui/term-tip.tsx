import * as React from 'react';

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';

type TermTipProps = {
  /** The abbreviation as it reads on screen, e.g. "GTD". */
  children: React.ReactNode;
  /** The words it stands for. Keep it to a sentence; anything longer is an `<ExplainerNote>` or a rules section. */
  label: React.ReactNode;
  /** Trigger element for a term that is not plain inline text, e.g. `<Badge variant="secondary" />`. Defaults to a dotted-underlined span. */
  render?: React.ComponentProps<typeof TooltipTrigger>['render'];
};

/** Help tier 1: a domain abbreviation with its meaning one hover or Tab away. Every abbreviation the app prints ships wrapped in this. */
export function TermTip({
  children,
  label,
  render = <span className="term-tip" />,
}: TermTipProps) {
  return (
    <Tooltip>
      {/* tabIndex reaches the span the way the default <button> trigger would. */}
      <TooltipTrigger data-slot="term-tip" render={render} tabIndex={0}>
        {children}
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
