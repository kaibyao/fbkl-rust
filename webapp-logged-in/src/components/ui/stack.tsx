import { mergeProps } from '@base-ui/react/merge-props';
import { useRender } from '@base-ui/react/use-render';
import { cva } from 'class-variance-authority';

import { cn } from '@/lib/utils';

/** Main-axis direction of the flex container. */
export enum StackDirection {
  /** Vertical stack (flex-col). Default. */
  Col = 'col',
  /** Horizontal row (flex-row). */
  Row = 'row',
}

/** Gap between children — semantic spacing scale, not one-off gap-N values. */
export enum StackGap {
  /** No gap. Default. */
  None = 'none',
  /** 4px (gap-1). */
  Xs = 'xs',
  /** 8px (gap-2). */
  Sm = 'sm',
  /** 12px (gap-3). */
  Md = 'md',
  /** 16px (gap-4). */
  Lg = 'lg',
  /** 24px (gap-6). */
  Xl = 'xl',
}

/** Cross-axis alignment (align-items). */
export enum StackAlign {
  /** Pack to the cross-start edge. */
  Start = 'start',
  /** Center on the cross axis. */
  Center = 'center',
  /** Pack to the cross-end edge. */
  End = 'end',
  /** Fill the cross axis. */
  Stretch = 'stretch',
  /** Align text baselines. */
  Baseline = 'baseline',
}

/** Main-axis distribution (justify-content). */
export enum StackJustify {
  /** Pack to the main-start edge. */
  Start = 'start',
  /** Center on the main axis. */
  Center = 'center',
  /** Pack to the main-end edge. */
  End = 'end',
  /** Equal space between children. */
  Between = 'between',
  /** Equal space around children. */
  Around = 'around',
  /** Equal space between and around children. */
  Evenly = 'evenly',
}

const directionClass: Record<StackDirection, string> = {
  [StackDirection.Col]: 'stack--col',
  [StackDirection.Row]: 'stack--row',
};
const gapClass: Record<StackGap, string> = {
  [StackGap.None]: '',
  [StackGap.Xs]: 'stack--gap-xs',
  [StackGap.Sm]: 'stack--gap-sm',
  [StackGap.Md]: 'stack--gap-md',
  [StackGap.Lg]: 'stack--gap-lg',
  [StackGap.Xl]: 'stack--gap-xl',
};
const alignClass: Record<StackAlign, string> = {
  [StackAlign.Start]: 'stack--align-start',
  [StackAlign.Center]: 'stack--align-center',
  [StackAlign.End]: 'stack--align-end',
  [StackAlign.Stretch]: 'stack--align-stretch',
  [StackAlign.Baseline]: 'stack--align-baseline',
};
const justifyClass: Record<StackJustify, string> = {
  [StackJustify.Start]: 'stack--justify-start',
  [StackJustify.Center]: 'stack--justify-center',
  [StackJustify.End]: 'stack--justify-end',
  [StackJustify.Between]: 'stack--justify-between',
  [StackJustify.Around]: 'stack--justify-around',
  [StackJustify.Evenly]: 'stack--justify-evenly',
};

const stackVariants = cva('stack', {
  variants: {
    direction: directionClass,
    gap: gapClass,
    align: alignClass,
    justify: justifyClass,
    wrap: {
      true: 'stack--wrap',
    },
  },
  defaultVariants: {
    direction: StackDirection.Col,
    gap: StackGap.None,
  },
});

type StackProps = useRender.ComponentProps<'div'> & {
  /** Main-axis direction. Default `Col`. */
  direction?: StackDirection;
  /** Gap between children (semantic spacing scale). Default `None`. */
  gap?: StackGap;
  /** Cross-axis alignment (align-items). */
  align?: StackAlign;
  /** Main-axis distribution (justify-content). */
  justify?: StackJustify;
  /** Wrap children onto multiple lines. */
  wrap?: boolean;
};

export function Stack({
  className,
  direction,
  gap,
  align,
  justify,
  wrap,
  render,
  ...props
}: StackProps) {
  return useRender({
    defaultTagName: 'div',
    props: mergeProps<'div'>(
      {
        className: cn(
          stackVariants({ direction, gap, align, justify, wrap }),
          className,
        ),
      },
      props,
    ),
    render,
    state: {
      slot: 'stack',
    },
  });
}
