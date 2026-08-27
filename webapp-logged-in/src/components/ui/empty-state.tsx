import * as React from 'react';

import { Typography, TypographyVariant } from '@/components/ui/typography';
import { cn } from '@/lib/utils';

type EmptyStateProps = React.ComponentProps<'div'> & {
  /** Lucide icon element, shown in a muted zinc tile above the title. */
  icon?: React.ReactNode;
  /** What is missing, in the user's words ("No players yet"). */
  title: React.ReactNode;
  /** One line: why it is empty and what fills it. */
  description?: React.ReactNode;
  /** Optional `<Button>` that fills it. */
  action?: React.ReactNode;
};

/** Centered zero-data block. Stays neutral zinc on its own; a screen that wants a hype moment wraps it in a treated photo. */
export function EmptyState({
  className,
  icon,
  title,
  description,
  action,
  ...props
}: EmptyStateProps) {
  return (
    <div
      data-slot="empty-state"
      className={cn('empty-state', className)}
      {...props}
    >
      {icon ? (
        <div
          data-slot="empty-state-icon"
          aria-hidden="true"
          className="empty-state-icon"
        >
          {icon}
        </div>
      ) : null}
      <Typography
        variant={TypographyVariant.Heading3}
        data-slot="empty-state-title"
      >
        {title}
      </Typography>
      {description ? (
        <Typography
          variant={TypographyVariant.Muted}
          data-slot="empty-state-description"
          className="empty-state-description"
        >
          {description}
        </Typography>
      ) : null}
      {action}
    </div>
  );
}
