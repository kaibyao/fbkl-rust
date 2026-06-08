import ListIcon from '@mui/icons-material/List';
import Box from '@mui/material/Box';
import Drawer from '@mui/material/Drawer';
import List from '@mui/material/List';
import ListItem from '@mui/material/ListItem';
import ListItemButton from '@mui/material/ListItemButton';
import ListItemIcon from '@mui/material/ListItemIcon';
import ListItemText from '@mui/material/ListItemText';
import Toolbar from '@mui/material/Toolbar';
import Typography from '@mui/material/Typography';
import { FunctionComponent } from 'react';

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
        </List>
      </Box>
    </Drawer>
  );
};
