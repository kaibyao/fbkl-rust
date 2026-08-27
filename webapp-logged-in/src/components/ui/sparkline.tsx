import { cn } from '@/lib/utils';

/** The only two sparkline sizes the board allows: 28px in a row, 48px in a stat tile. Anything taller is a chart and owes the reader an axis. */
const SPARKLINE_SIZES = {
  row: { width: 96, height: 28 },
  tile: { width: 160, height: 48 },
} as const;

const EDGE_PADDING = 4;

/** Props for {@link Sparkline}. */
export interface SparklineProps {
  /** Oldest value first. The same span across every row of a list, so the shapes compare. */
  values: number[];
  /** Bars for counts, a line for a level that moves. */
  variant?: 'line' | 'bars';
  /** `row` is 28px, `tile` is 48px. */
  size?: 'row' | 'tile';
  /** True when the last point is the current one, which turns the end marker orange. */
  isLive?: boolean;
  className?: string;
}

/** A shape, not a chart: no axis, no gridlines, no baseline, no labels and no interaction of any kind. It is hidden from screen readers because it never holds the only copy of a number, so always print the current value next to it. */
export function Sparkline({
  values,
  variant = 'line',
  size = 'row',
  isLive = false,
  className,
}: SparklineProps) {
  if (values.length < 2) {
    return null;
  }

  const { width, height } = SPARKLINE_SIZES[size];
  const lowest = Math.min(...values);
  const highest = Math.max(...values);
  const valueSpan = highest - lowest;
  const plotHeight = height - EDGE_PADDING * 2;
  const toY = (value: number) =>
    valueSpan === 0
      ? height / 2
      : height - EDGE_PADDING - ((value - lowest) / valueSpan) * plotHeight;
  const endFill = isLive ? 'var(--primary-hot)' : 'var(--chart-2)';
  const lastIndex = values.length - 1;
  const stepX = (width - EDGE_PADDING * 2) / lastIndex;
  const bandWidth = (width - EDGE_PADDING * 2) / values.length;

  return (
    <svg
      data-slot="sparkline"
      className={cn('sparkline', className)}
      viewBox={`0 0 ${width} ${height}`}
      width={width}
      height={height}
      aria-hidden="true"
    >
      {variant === 'line' && (
        <polyline
          className="sparkline-series"
          points={values
            .map(
              (value, index) => `${EDGE_PADDING + index * stepX},${toY(value)}`,
            )
            .join(' ')}
        />
      )}
      {variant === 'line' && (
        <circle
          className="sparkline-dot"
          cx={width - EDGE_PADDING}
          cy={toY(values[lastIndex])}
          r={3}
          fill={endFill}
        />
      )}
      {variant === 'bars' &&
        values.map((value, index) => (
          <rect
            // Positional series: an index is the only identity a point has.
            key={index}
            className="sparkline-bar"
            x={EDGE_PADDING + index * bandWidth}
            y={toY(value)}
            width={Math.max(bandWidth - 2, 1)}
            height={Math.max(height - EDGE_PADDING - toY(value), 1)}
            fill={index === lastIndex ? endFill : undefined}
          />
        ))}
    </svg>
  );
}
