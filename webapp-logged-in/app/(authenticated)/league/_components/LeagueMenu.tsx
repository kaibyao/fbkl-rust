'use client';

import { FunctionComponent } from 'react';
import { useRouter } from 'next/navigation';
import AttachMoneyIcon from '@mui/icons-material/AttachMoney';
import Box from '@mui/material/Box';
import CurrencyExchangeIcon from '@mui/icons-material/CurrencyExchange';
import Drawer from '@mui/material/Drawer';
import List from '@mui/material/List';
import ListItem from '@mui/material/ListItem';
import ListItemButton from '@mui/material/ListItemButton';
import ListItemIcon from '@mui/material/ListItemIcon';
import ListItemText from '@mui/material/ListItemText';
import Toolbar from '@mui/material/Toolbar';
import Typography from '@mui/material/Typography';
// import DashboardIcon from "@mui/icons-material/Dashboard";
import ListIcon from '@mui/icons-material/List';
import LocalActivityIcon from '@mui/icons-material/LocalActivity';
import PersonIcon from '@mui/icons-material/Person';
import TransferWithinAStationIcon from '@mui/icons-material/TransferWithinAStation';
import ScheduleIcon from '@mui/icons-material/Schedule';

export const LEAGUE_MENU_WIDTH_PX = 240;
export const LEAGUE_MENU_WIDTH_SX = LEAGUE_MENU_WIDTH_PX / 8;

export const LeagueMenu: FunctionComponent = () => {
  const router = useRouter();

  const handleDeadlineConfigClick = () => {
    router.push('/league/deadline-config');
  };

  return (
    <Drawer
      variant="permanent"
      sx={{
        width: LEAGUE_MENU_WIDTH_PX,
        flexShrink: 0,
        [`& .MuiDrawer-paper`]: {
          width: LEAGUE_MENU_WIDTH_PX,
          boxSizing: 'border-box',
        },
      }}
    >
      <Toolbar />
      <Box>
        <List disablePadding>
          {/* <ListItem disablePadding>
            <ListItemButton>
              <ListItemIcon>
                <DashboardIcon />
              </ListItemIcon>
              <ListItemText primary="Dashboard" />
            </ListItemButton>
          </ListItem> */}
          <ListItem disablePadding>
            <ListItemButton>
              <ListItemIcon>
                <ListIcon />
              </ListItemIcon>
              <ListItemText
                primary={
                  <Typography variant="body1" color="yellow">
                    Rosters
                  </Typography>
                }
                disableTypography
              />
            </ListItemButton>
          </ListItem>
          <ListItem disablePadding>
            <ListItemButton onClick={handleDeadlineConfigClick}>
              <ListItemIcon>
                <ScheduleIcon />
              </ListItemIcon>
              <ListItemText primary="Deadlines" />
            </ListItemButton>
          </ListItem>
          {/* <ListItem disablePadding>
            <ListItemButton>
              <ListItemIcon>
                <AttachMoneyIcon />
              </ListItemIcon>
              <ListItemText primary="Auctions" />
            </ListItemButton>
          </ListItem>
          <ListItem disablePadding>
            <ListItemButton>
              <ListItemIcon>
                <PersonIcon />
              </ListItemIcon>
              <ListItemText primary="Players" />
            </ListItemButton>
          </ListItem>
          <ListItem disablePadding>
            <ListItemButton>
              <ListItemIcon>
                <LocalActivityIcon />
              </ListItemIcon>
              <ListItemText primary="Draft Picks" />
            </ListItemButton>
          </ListItem>
          <ListItem disablePadding>
            <ListItemButton>
              <ListItemIcon>
                <TransferWithinAStationIcon />
              </ListItemIcon>
              <ListItemText primary="Trades" />
            </ListItemButton>
          </ListItem>
          <ListItem disablePadding>
            <ListItemButton>
              <ListItemIcon>
                <CurrencyExchangeIcon />
              </ListItemIcon>
              <ListItemText primary="Transactions" />
            </ListItemButton>
          </ListItem> */}
        </List>
      </Box>
    </Drawer>
  );
};
