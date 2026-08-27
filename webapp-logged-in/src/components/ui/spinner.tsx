import { cva, type VariantProps } from 'class-variance-authority';
import { Loader2 } from 'lucide-react';
import * as React from 'react';

import { cn } from '@/lib/utils';

const spinnerVariants = cva('spinner', {
  variants: {
    size: {
      sm: 'spinner--sm',
      default: 'spinner--default',
      lg: 'spinner--lg',
    },
  },
  defaultVariants: {
    size: 'default',
  },
});

/** Inline loading glyph for buttons and inline actions. Pair it with `disabled`. Lists, rows and cards use `<Skeleton>` instead, so the page keeps its shape while it loads. */
export function Spinner({
  className,
  size,
  ...props
}: React.ComponentProps<'svg'> & VariantProps<typeof spinnerVariants>) {
  return (
    <Loader2
      data-slot="spinner"
      role="status"
      aria-label="Loading"
      className={cn(spinnerVariants({ size }), className)}
      {...props}
    />
  );
}
