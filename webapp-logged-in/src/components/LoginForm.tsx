import { useNavigate } from '@tanstack/react-router';
import { Loader2 } from 'lucide-react';
import { useState } from 'react';
import { SubmitHandler, useForm } from 'react-hook-form';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
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
    <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col gap-5">
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="email">Email</Label>
        <Input
          id="email"
          autoComplete="email"
          type="email"
          aria-invalid={!!formErrors.email}
          {...register('email', { required: true })}
        />
      </div>

      <div className="flex flex-col gap-1.5">
        <Label htmlFor="password">Password</Label>
        <Input
          id="password"
          autoComplete="current-password"
          type="password"
          aria-invalid={!!formErrors.password}
          {...register('password', { required: true })}
        />
      </div>

      <Button
        type="submit"
        size="lg"
        disabled={isSubmitting}
        className="w-full hover:bg-primary-hot"
      >
        {isSubmitting && <Loader2 className="animate-spin" />}
        Sign in
      </Button>

      {loginError && <p className="text-xs text-destructive">{loginError}</p>}
    </form>
  );
};
