import Button from '@mui/material/Button';
import CircularProgress from '@mui/material/CircularProgress';
import FormHelperText from '@mui/material/FormHelperText';
import Stack from '@mui/material/Stack';
import TextField from '@mui/material/TextField';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { SubmitHandler, useForm } from 'react-hook-form';
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
    <form onSubmit={handleSubmit(onSubmit)}>
      <Stack spacing={3}>
        <TextField
          autoComplete="email"
          error={!!formErrors.email}
          fullWidth
          label="Email"
          type="email"
          {...register('email', { required: true })}
          slotProps={{ inputLabel: { shrink: true } }}
        />
        <TextField
          autoComplete="current-password"
          error={!!formErrors.password}
          fullWidth
          label="Password"
          type="password"
          {...register('password', { required: true })}
          slotProps={{ inputLabel: { shrink: true } }}
        />
        <Button
          type="submit"
          disabled={isSubmitting}
          variant="contained"
          startIcon={
            isSubmitting ? (
              <CircularProgress size="1em" sx={{ mr: 1 }} />
            ) : undefined
          }
        >
          Submit
        </Button>
        {loginError && <FormHelperText error>{loginError}</FormHelperText>}
      </Stack>
    </form>
  );
};
