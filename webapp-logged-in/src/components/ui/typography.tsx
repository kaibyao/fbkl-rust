import { mergeProps } from '@base-ui/react/merge-props';
import { useRender } from '@base-ui/react/use-render';
import { cva } from 'class-variance-authority';
import * as React from 'react';

import { cn } from '@/lib/utils';

/** Text style preset. `Heading{N}` render `<h{N}>` by default; override the tag with `render`. */
export enum TypographyVariant {
  /** Hero display — 4xl black, splash/marketing only. Renders `<h1>`. */
  Display = 'display',
  /** Page title — 3xl black, one per page. Renders `<h1>`. */
  Heading1 = 'heading1',
  /** Section / card title — base bold. Renders `<h2>`. */
  Heading2 = 'heading2',
  /** Sub-section title — sm bold. Renders `<h3>`. */
  Heading3 = 'heading3',
  /** Minor heading — xs bold. Renders `<h4>`. */
  Heading4 = 'heading4',
  /** Numeric stat / figure — sm bold, tabular-nums. Renders `<span>`. */
  Stat = 'stat',
  /** Eyebrow / kicker — xs uppercase, wide tracking, primary-hot. Renders `<p>`. */
  Eyebrow = 'eyebrow',
  /** Uppercase muted label above a list/group. Renders `<h3>`. */
  SectionLabel = 'section-label',
  /** Default body copy — sm. Renders `<p>`. */
  Body = 'body',
  /** Compact body copy — xs. Renders `<p>`. */
  BodySm = 'body-sm',
  /** Emphasized compact body — xs medium. Renders `<p>`. */
  BodyStrong = 'body-strong',
  /** Muted secondary text — sm. Renders `<p>`. */
  Muted = 'muted',
  /** Muted secondary text, compact — xs. Renders `<p>`. */
  MutedSm = 'muted-sm',
  /** Inline muted recolor; inherits parent size. Renders `<span>`. */
  InlineMuted = 'inline-muted',
  /** Error message — sm destructive. Renders `<p>`. */
  Error = 'error',
  /** Error message, compact — xs destructive. Renders `<p>`. */
  ErrorSm = 'error-sm',
}

// Both maps keyed by TypographyVariant so a missing/extra entry is a compile error.
const classByVariant: Record<TypographyVariant, string> = {
  [TypographyVariant.Display]: 'typography-display',
  [TypographyVariant.Heading1]: 'typography-heading1',
  [TypographyVariant.Heading2]: 'typography-heading2',
  [TypographyVariant.Heading3]: 'typography-heading3',
  [TypographyVariant.Heading4]: 'typography-heading4',
  [TypographyVariant.Stat]: 'typography-stat',
  [TypographyVariant.Eyebrow]: 'typography-eyebrow',
  [TypographyVariant.SectionLabel]: 'typography-section-label',
  [TypographyVariant.Body]: 'typography-body',
  [TypographyVariant.BodySm]: 'typography-body-sm',
  [TypographyVariant.BodyStrong]: 'typography-body-strong',
  [TypographyVariant.Muted]: 'typography-muted',
  [TypographyVariant.MutedSm]: 'typography-muted-sm',
  [TypographyVariant.InlineMuted]: 'typography-inline-muted',
  [TypographyVariant.Error]: 'typography-error',
  [TypographyVariant.ErrorSm]: 'typography-error-sm',
};

const defaultTagByVariant: Record<
  TypographyVariant,
  keyof React.JSX.IntrinsicElements
> = {
  [TypographyVariant.Display]: 'h1',
  [TypographyVariant.Heading1]: 'h1',
  [TypographyVariant.Heading2]: 'h2',
  [TypographyVariant.Heading3]: 'h3',
  [TypographyVariant.Heading4]: 'h4',
  [TypographyVariant.Stat]: 'span',
  [TypographyVariant.Eyebrow]: 'p',
  [TypographyVariant.SectionLabel]: 'h3',
  [TypographyVariant.Body]: 'p',
  [TypographyVariant.BodySm]: 'p',
  [TypographyVariant.BodyStrong]: 'p',
  [TypographyVariant.Muted]: 'p',
  [TypographyVariant.MutedSm]: 'p',
  [TypographyVariant.InlineMuted]: 'span',
  [TypographyVariant.Error]: 'p',
  [TypographyVariant.ErrorSm]: 'p',
};

export const typographyVariants = cva('', {
  variants: {
    variant: classByVariant,
  },
  defaultVariants: {
    variant: TypographyVariant.Body,
  },
});

type TypographyProps = useRender.ComponentProps<'p'> & {
  /** Text style preset — pick a `TypographyVariant.*` member (hover each for its size/weight/tag). */
  variant?: TypographyVariant;
};

export function Typography({
  className,
  variant = TypographyVariant.Body,
  render,
  ...props
}: TypographyProps) {
  return useRender({
    defaultTagName: defaultTagByVariant[variant],
    props: mergeProps<'p'>(
      {
        className: cn(typographyVariants({ variant }), className),
      },
      props,
    ),
    render,
    state: {
      slot: 'typography',
      variant,
    },
  });
}
