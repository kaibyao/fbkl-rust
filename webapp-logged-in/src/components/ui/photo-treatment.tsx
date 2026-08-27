import { cva, type VariantProps } from 'class-variance-authority';
import * as React from 'react';

import { cn } from '@/lib/utils';

const photoTreatmentVariants = cva('photo-treatment', {
  variants: {
    variant: {
      scrim: 'photo-treatment--scrim',
      duotone: 'photo-treatment--duotone',
      spotlight: 'photo-treatment--spotlight',
      analytical: 'photo-treatment--analytical',
    },
  },
  defaultVariants: {
    variant: 'scrim',
  },
});

/**
 * Puts a photo on the dark zinc base so the orange stays the loudest thing on screen. `scrim` is the
 * default treatment for player cards, `duotone` is for hype and marketing moments, `spotlight` is for a single
 * player of the week, `analytical` is for data-heavy screens. Children sit above the treatment layer,
 * so a label goes in as a child.
 *
 * Label contrast rule, the same for all four variants: text over the photo goes in a child with
 * `className="photo-treatment-label"`, which paints its own scrim, so the text always sits on an
 * opaque `--background` whatever the photo does and whatever size the card is. The sum is therefore
 * one fixed number, `--foreground` on `--background`, 19:1, and the check is to read the label's
 * computed background and see an opaque `--background` at the bottom stop. Text that cannot take a
 * scrim does not go on a photo.
 */
export function PhotoTreatment({
  className,
  variant,
  src,
  alt,
  children,
  ...props
}: React.ComponentProps<'div'> &
  VariantProps<typeof photoTreatmentVariants> & {
    src: string;
    alt: string;
  }) {
  return (
    <div
      data-slot="photo-treatment"
      className={cn(photoTreatmentVariants({ variant }), className)}
      {...props}
    >
      <img src={src} alt={alt} />
      {children}
    </div>
  );
}
