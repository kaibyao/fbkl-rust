import { createFileRoute } from '@tanstack/react-router';
import { LoginForm } from '@/components/LoginForm';

export const Route = createFileRoute('/login')({
  component: LoginPage,
});

// Placeholder hero image — swap for a licensed/team-provided asset before shipping.
const HERO_IMG =
  'https://cdn.nba.com/manage/2025/12/GettyImages-2220888872-1024x1536.jpg';

function LoginPage() {
  return (
    <div className="relative flex min-h-svh items-center justify-center overflow-hidden bg-background px-4">
      {/* Duotone NBA backdrop (treatment B) */}
      <div
        className="absolute inset-0 bg-cover bg-center opacity-30 grayscale"
        style={{ backgroundImage: `url(${HERO_IMG})` }}
        aria-hidden
      />
      <div
        className="absolute inset-0"
        aria-hidden
        style={{
          background:
            'radial-gradient(900px 500px at 85% 5%, color-mix(in oklch, var(--primary) 35%, transparent), transparent 60%), linear-gradient(to top, var(--background) 8%, color-mix(in oklch, var(--background) 60%, transparent) 60%)',
        }}
      />

      <div className="relative w-full max-w-sm">
        <p className="text-xs font-semibold tracking-[0.22em] text-primary-hot uppercase">
          FBKL
        </p>
        <h1 className="mt-2 mb-6 font-heading text-4xl font-black tracking-tight">
          Welcome back.
        </h1>
        <LoginForm />
      </div>
    </div>
  );
}
