import {
  Box,
  Drawer,
  List,
  ListItem,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Toolbar,
  Typography,
} from '@mui/material';
import { FunctionComponent } from 'react';
import AttachMoneyIcon from '@mui/icons-material/AttachMoney';
import CurrencyExchangeIcon from '@mui/icons-material/CurrencyExchange';
// import DashboardIcon from "@mui/icons-material/Dashboard";
import ListIcon from '@mui/icons-material/List';
import LocalActivityIcon from '@mui/icons-material/LocalActivity';
import PersonIcon from '@mui/icons-material/Person';
import TransferWithinAStationIcon from '@mui/icons-material/TransferWithinAStation';

export const LEAGUE_MENU_WIDTH = 240;

export const LeagueMenu: FunctionComponent = () => {
  return (
    <Drawer
      variant="permanent"
      sx={{
        width: LEAGUE_MENU_WIDTH,
        flexShrink: 0,
        [`& .MuiDrawer-paper`]: {
          width: LEAGUE_MENU_WIDTH,
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
          </ListItem>
        </List>
      </Box>
    </Drawer>
  );
};
