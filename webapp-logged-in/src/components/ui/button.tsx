import { Button as ButtonPrimitive } from '@base-ui/react/button';
import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '@/lib/utils';

const buttonVariants = cva('group/button btn', {
  variants: {
    variant: {
      default: 'btn--default',
      outline: 'btn--outline',
      secondary: 'btn--secondary',
      ghost: 'btn--ghost',
      destructive: 'btn--destructive',
      link: 'btn--link',
    },
    size: {
      default: 'btn--size-default',
      xs: 'btn--size-xs',
      sm: 'btn--size-sm',
      lg: 'btn--size-lg',
      icon: 'btn--size-icon',
      'icon-xs': 'btn--size-icon-xs',
      'icon-sm': 'btn--size-icon-sm',
      'icon-lg': 'btn--size-icon-lg',
    },
  },
  defaultVariants: {
    variant: 'default',
    size: 'default',
  },
});

function Button({
  className,
  variant = 'default',
  size = 'default',
  ...props
}: ButtonPrimitive.Props & VariantProps<typeof buttonVariants>) {
  return (
    <ButtonPrimitive
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  );
}

export { Button, buttonVariants };
