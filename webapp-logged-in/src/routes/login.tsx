import Stack from '@mui/material/Stack';
import Typography from '@mui/material/Typography';
import { createFileRoute } from '@tanstack/react-router';
import { LoginForm } from '@/components/LoginForm';

export const Route = createFileRoute('/login')({
  component: LoginPage,
});

function LoginPage() {
  return (
    <Stack spacing={3}>
      <Typography variant="h2">Login</Typography>
      <LoginForm />
    </Stack>
  );
}
