import { Info, X } from 'lucide-react';
import * as React from 'react';

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

type ExplainerNoteProps = React.ComponentProps<'div'> & {
  /** Optional link or `<Button>` into the matching League rules section. */
  action?: React.ReactNode;
  /** Shows a dismiss button that calls this. The note stores nothing: the caller owns where "dismissed" lives. */
  onDismiss?: () => void;
};

/** Help tier 2: one zinc line explaining a league rule at the moment it bites. One sentence of a real rule, never a paragraph. */
export function ExplainerNote({
  action,
  children,
  className,
  onDismiss,
  ...props
}: ExplainerNoteProps) {
  return (
    <div
      data-slot="explainer-note"
      className={cn('explainer-note', className)}
      {...props}
    >
      <span
        data-slot="explainer-note-icon"
        aria-hidden="true"
        className="explainer-note-icon"
      >
        <Info />
      </span>
      <div className="min-w-0 flex-1">{children}</div>
      {action}
      {onDismiss ? (
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Dismiss"
          className="shrink-0"
          onClick={onDismiss}
        >
          <X aria-hidden="true" />
        </Button>
      ) : null}
    </div>
  );
}
