import { Toast as ToastPrimitive } from '@base-ui/react/toast';
import {
  CircleCheckIcon,
  InfoIcon,
  OctagonXIcon,
  TriangleAlertIcon,
  XIcon,
} from 'lucide-react';
import * as React from 'react';

import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/utils';

/** What the toast is telling you. Picks the tint, the glyph, and the word screen readers hear, so colour never carries the meaning alone. */
export enum ToastKind {
  /** Something worked. Green `--success` tint. */
  Success = 'success',
  /** Something failed and the user can retry. Red `--destructive` tint. */
  Error = 'error',
  /** Neutral news. Plain card surface. Default for an unrecognised kind. */
  Info = 'info',
  /** Something needs attention but nothing failed yet. Red tint, warning glyph. */
  Warning = 'warning',
  /** Work in flight. Plain card surface, spinning glyph. */
  Loading = 'loading',
}

type ToastIconComponent = React.ComponentType<{
  className?: string;
  'aria-hidden'?: boolean;
}>;

// One record per kind so a new kind cannot ship without its tint, glyph and word.
const styleByKind: Record<
  ToastKind,
  { className: string; Icon: ToastIconComponent; word: string }
> = {
  [ToastKind.Success]: {
    className: 'toast--success',
    Icon: CircleCheckIcon,
    word: 'Success',
  },
  [ToastKind.Error]: {
    className: 'toast--destructive',
    Icon: OctagonXIcon,
    word: 'Error',
  },
  [ToastKind.Info]: {
    className: 'toast--default',
    Icon: InfoIcon,
    word: 'Info',
  },
  [ToastKind.Warning]: {
    className: 'toast--destructive',
    Icon: TriangleAlertIcon,
    word: 'Warning',
  },
  [ToastKind.Loading]: {
    className: 'toast--default',
    Icon: Spinner,
    word: 'Loading',
  },
};

// Base UI carries `type` as a free-form string; anything we do not style reads as Info.
function toToastKind(type: string | undefined): ToastKind {
  return type != null && type in styleByKind
    ? (type as ToastKind)
    : ToastKind.Info;
}

/** Raise a toast from anywhere under `<Toaster>`: `toast.add({ type: ToastKind.Error, title, description })`. */
export const toast = ToastPrimitive.createToastManager();

/** Reads the live toast list plus `add`/`close`/`update` from the nearest `<Toaster>`. */
export const useToastManager = ToastPrimitive.useToastManager;

function ToastProvider({ ...props }: ToastPrimitive.Provider.Props) {
  return <ToastPrimitive.Provider {...props} />;
}

function ToastPortal({ ...props }: ToastPrimitive.Portal.Props) {
  return <ToastPrimitive.Portal data-slot="toast-portal" {...props} />;
}

function ToastViewport({ className, ...props }: ToastPrimitive.Viewport.Props) {
  return (
    <ToastPrimitive.Viewport
      data-slot="toast-viewport"
      className={cn('toast-viewport', className)}
      {...props}
    />
  );
}

/** One toast surface. `kind` picks the tint; `<ToastIcon>` carries the matching glyph and word. */
export function Toast({
  className,
  kind = ToastKind.Info,
  ...props
}: ToastPrimitive.Root.Props & { kind?: ToastKind }) {
  return (
    <ToastPrimitive.Root
      data-slot="toast"
      data-kind={kind}
      className={cn(
        'group/toast toast',
        styleByKind[kind].className,
        className,
      )}
      {...props}
    />
  );
}

function ToastContent({ className, ...props }: ToastPrimitive.Content.Props) {
  return (
    <ToastPrimitive.Content
      data-slot="toast-content"
      className={cn('toast-content', className)}
      {...props}
    />
  );
}

function ToastTitle({ className, ...props }: ToastPrimitive.Title.Props) {
  return (
    <ToastPrimitive.Title
      data-slot="toast-title"
      className={cn('toast-title', className)}
      {...props}
    />
  );
}

function ToastDescription({
  className,
  ...props
}: ToastPrimitive.Description.Props) {
  return (
    <ToastPrimitive.Description
      data-slot="toast-description"
      className={cn('toast-description', className)}
      {...props}
    />
  );
}

function ToastAction({
  className,
  render = <Button variant="outline" size="sm" />,
  ...props
}: ToastPrimitive.Action.Props) {
  return (
    <ToastPrimitive.Action
      data-slot="toast-action"
      render={render}
      className={cn('toast-action', className)}
      {...props}
    />
  );
}

function ToastClose({
  className,
  children,
  render = <Button variant="ghost" size="icon-sm" />,
  ...props
}: ToastPrimitive.Close.Props) {
  return (
    <ToastPrimitive.Close
      data-slot="toast-close"
      aria-label="Close toast"
      render={render}
      className={cn('toast-close', className)}
      {...props}
    >
      {children ?? <XIcon aria-hidden="true" />}
    </ToastPrimitive.Close>
  );
}

function ToastIcon({ kind }: { kind: ToastKind }) {
  const { Icon, word } = styleByKind[kind];

  return (
    <span data-slot="toast-icon" className="toast-icon">
      <Icon aria-hidden />
      <span className="sr-only">{word}</span>
    </span>
  );
}

function ToastList() {
  const { toasts } = useToastManager();

  return toasts.map((toastItem) => {
    const kind = toToastKind(toastItem.type);

    return (
      <Toast key={toastItem.id} toast={toastItem} kind={kind}>
        <ToastContent>
          <ToastIcon kind={kind} />
          <div className="min-w-0 flex-1">
            <ToastTitle />
            <ToastDescription />
          </div>
          <ToastAction />
          <ToastClose />
        </ToastContent>
      </Toast>
    );
  });
}

/** Mount once near the app root; every `toast.add(...)` call renders through it. */
export function Toaster({
  children,
  toastManager = toast,
  ...props
}: ToastPrimitive.Provider.Props) {
  return (
    <ToastProvider toastManager={toastManager} {...props}>
      {children}
      <ToastPortal>
        <ToastViewport>
          <ToastList />
        </ToastViewport>
      </ToastPortal>
    </ToastProvider>
  );
}
