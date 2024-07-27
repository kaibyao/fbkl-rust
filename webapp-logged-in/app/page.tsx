import { AppProviders } from '@/app/_components/AppProviders';

export default function Page({ children }: { children: React.ReactNode }) {
  return <AppProviders>{children}</AppProviders>;
}
