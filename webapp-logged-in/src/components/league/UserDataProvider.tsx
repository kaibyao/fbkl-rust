import { createContext, FC, PropsWithChildren, useContext } from 'react';
import { LoggedIn } from '@/lib/auth';

const UserDataContext = createContext<LoggedIn | undefined>(undefined);

interface Props {
  userData: LoggedIn;
}

export const UserDataProvider: FC<PropsWithChildren<Props>> = ({
  children,
  userData,
}) => (
  <UserDataContext.Provider value={userData}>
    {children}
  </UserDataContext.Provider>
);

export const useUserData = (): LoggedIn => {
  const context = useContext(UserDataContext);
  if (context === undefined) {
    throw new Error('useUserData must be used within a UserDataProvider');
  }
  return context;
};
