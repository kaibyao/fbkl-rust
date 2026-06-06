import { createContext, FunctionComponent, PropsWithChildren } from 'react';
import { LoggedIn } from '@/lib/auth';

type LoggedInUserContext = Omit<LoggedIn, 'isLoggedIn'>;

export const UserContext = createContext<LoggedInUserContext>({
  email: '',
  userId: 0,
  selectedLeagueId: 0,
});

interface Props {
  user: LoggedInUserContext;
}

export const UserProvider: FunctionComponent<PropsWithChildren<Props>> = ({
  children,
  user,
}) => <UserContext.Provider value={user}>{children}</UserContext.Provider>;
