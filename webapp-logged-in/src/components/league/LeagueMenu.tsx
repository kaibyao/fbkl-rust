import { ClipboardList } from 'lucide-react';
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

export const LeagueMenu: FunctionComponent = () => {
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
            <SidebarMenuItem>
              <SidebarMenuButton isActive tooltip="Rosters">
                <ClipboardList />
                <span>Rosters</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>
  );
};
