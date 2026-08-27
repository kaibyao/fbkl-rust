'use client';

import { Progress as ProgressPrimitive } from '@base-ui/react/progress';

import { cn } from '@/lib/utils';

/** Value bar for an amount against a known maximum: salary against the cap, roster slots filled. `label` names what the bar measures for screen readers, since the bar itself only shows a shape. */
export function Progress({
  className,
  label,
  ...props
}: ProgressPrimitive.Root.Props & { label: string }) {
  return (
    <ProgressPrimitive.Root
      data-slot="progress"
      className={cn('progress', className)}
      {...props}
    >
      <ProgressPrimitive.Label className="sr-only">
        {label}
      </ProgressPrimitive.Label>
      <ProgressPrimitive.Track
        data-slot="progress-track"
        className="progress-track"
      >
        <ProgressPrimitive.Indicator
          data-slot="progress-indicator"
          className="progress-indicator"
        />
      </ProgressPrimitive.Track>
    </ProgressPrimitive.Root>
  );
}
