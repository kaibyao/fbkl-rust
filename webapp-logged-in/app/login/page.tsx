import { LoginForm } from '@/app/login/_components/LoginForm';
import Stack from '@mui/material/Stack';
import Typography from '@mui/material/Typography';

export default function LoginPage() {
  return (
    <Stack spacing={3}>
      <Typography variant="h2">Login</Typography>
      <LoginForm />
    </Stack>
  );
}
