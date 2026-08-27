import { mergeProps } from '@base-ui/react/merge-props';
import { useRender } from '@base-ui/react/use-render';
import { cva, type VariantProps } from 'class-variance-authority';
import * as React from 'react';

import { cn } from '@/lib/utils';

/** `tint` is the 20% primary wash for a status that wants warmth without a solid fill. `pill` is the full-round shape, and it is the only variant that takes a `<BadgeDot>`. */
const badgeVariants = cva('group/badge badge', {
  variants: {
    variant: {
      default: 'badge--default',
      secondary: 'badge--secondary',
      destructive: 'badge--destructive',
      outline: 'badge--outline',
      ghost: 'badge--ghost',
      link: 'badge--link',
      tint: 'badge--tint',
      pill: 'badge--pill',
    },
  },
  defaultVariants: {
    variant: 'default',
  },
});

function Badge({
  className,
  variant = 'default',
  render,
  ...props
}: useRender.ComponentProps<'span'> & VariantProps<typeof badgeVariants>) {
  return useRender({
    defaultTagName: 'span',
    props: mergeProps<'span'>(
      {
        className: cn(badgeVariants({ variant }), className),
      },
      props,
    ),
    render,
    state: {
      slot: 'badge',
      variant,
    },
  });
}

/** The live indicator inside a `pill` badge: a hot-orange dot that pulses while the thing it marks is happening now. It goes still under `prefers-reduced-motion`. */
function BadgeDot({ className, ...props }: React.ComponentProps<'span'>) {
  return (
    <span
      data-slot="badge-dot"
      aria-hidden="true"
      className={cn('badge-dot', className)}
      {...props}
    />
  );
}

export { Badge, BadgeDot, badgeVariants };
