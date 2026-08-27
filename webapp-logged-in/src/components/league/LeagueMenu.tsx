import { Link, useMatchRoute } from '@tanstack/react-router';
import { BookOpen, ClipboardList } from 'lucide-react';
import { FunctionComponent } from 'react';
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/components/ui/sidebar';

const LEAGUE_MENU_ITEMS = [
  { to: '/league', label: 'Rosters', icon: ClipboardList },
  { to: '/league/rules', label: 'League rules', icon: BookOpen },
] as const;

export const LeagueMenu: FunctionComponent = () => {
  const matchRoute = useMatchRoute();

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <span className="px-2 font-heading text-base font-black tracking-tight text-primary-hot group-data-[collapsible=icon]:hidden">
          FBKL
        </span>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>League</SidebarGroupLabel>
          <SidebarMenu>
            {LEAGUE_MENU_ITEMS.map(({ to, label, icon: Icon }) => (
              <SidebarMenuItem key={to}>
                <SidebarMenuButton
                  isActive={Boolean(matchRoute({ to }))}
                  tooltip={label}
                  render={<Link to={to} />}
                >
                  <Icon />
                  <span>{label}</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            ))}
          </SidebarMenu>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>
  );
};
