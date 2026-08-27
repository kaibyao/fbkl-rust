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

/** Puts a photo on the dark zinc base so the orange stays the loudest thing on screen. `scrim` is the workhorse for player cards, `duotone` is for hype and marketing moments, `spotlight` is for a single player of the week, `analytical` is for data-heavy screens. Children sit above the treatment layer, so a label goes in as a child. */
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
