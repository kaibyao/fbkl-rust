import { createRootRoute, Outlet } from '@tanstack/react-router';
import { AppProviders } from '@/components/AppProviders';

export const Route = createRootRoute({
  component: () => (
    <AppProviders>
      <Outlet />
    </AppProviders>
  ),
});
