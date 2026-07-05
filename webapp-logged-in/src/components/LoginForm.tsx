import { useNavigate } from '@tanstack/react-router';
import { Loader2 } from 'lucide-react';
import { useState } from 'react';
import { SubmitHandler, useForm } from 'react-hook-form';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Stack, StackGap } from '@/components/ui/stack';
import { Typography, TypographyVariant } from '@/components/ui/typography';
import { processLogin } from '@/lib/auth';

interface LoginFormFields {
  email: string;
  password: string;
}

export const LoginForm: React.FC = () => {
  const navigate = useNavigate();
  const [loginError, setLoginError] = useState<string | null>(null);
  const {
    formState: { errors: formErrors, isSubmitting },
    handleSubmit,
    register,
  } = useForm<LoginFormFields>({ mode: 'onBlur' });

  const onSubmit: SubmitHandler<LoginFormFields> = async (data) => {
    setLoginError(null);
    const success = await processLogin(data);
    if (success) {
      // Root route re-checks auth and routes to the right place.
      await navigate({ to: '/' });
    } else {
      setLoginError('Invalid email or password.');
    }
  };

  return (
    <Stack
      render={<form onSubmit={handleSubmit(onSubmit)} />}
      gap={StackGap.Lg}
    >
      <Stack gap={StackGap.Sm}>
        <Label htmlFor="email">Email</Label>
        <Input
          id="email"
          autoComplete="email"
          type="email"
          aria-invalid={!!formErrors.email}
          {...register('email', { required: true })}
        />
      </Stack>

      <Stack gap={StackGap.Sm}>
        <Label htmlFor="password">Password</Label>
        <Input
          id="password"
          autoComplete="current-password"
          type="password"
          aria-invalid={!!formErrors.password}
          {...register('password', { required: true })}
        />
      </Stack>

      <Button
        type="submit"
        size="lg"
        disabled={isSubmitting}
        className="w-full hover:bg-primary-hot"
      >
        {isSubmitting && <Loader2 className="animate-spin" />}
        Sign in
      </Button>

      {loginError && (
        <Typography variant={TypographyVariant.ErrorSm}>
          {loginError}
        </Typography>
      )}
    </Stack>
  );
};
