'use server';

import { BASE_API_URL } from '@/app/_lib/constants';
import authFetch from '@/app/_lib/api';

interface LoggedInDataRaw {
  id: number;
  email: string;
  selected_league_id: number;
}

type NotLoggedInResponse = { id: never };

type LoggedInResponseRaw = LoggedInDataRaw | NotLoggedInResponse;

export interface LoggedIn {
  isLoggedIn: true;
  email: string;
  userId: number;
  selectedLeagueId: number;
}

interface NotLoggedIn {
  isLoggedIn: false;
}

type LoggedInResponse = LoggedIn | NotLoggedIn;

const GET_USER_DATA_API_URL = `${BASE_API_URL}/api/user`;

export async function getUserData(): Promise<LoggedInResponse> {
  const rawResponsePromise = await authFetch(GET_USER_DATA_API_URL, {
    headers: {
      'Content-Type': 'application/json',
    },
  });

  if (rawResponsePromise.status !== 200) {
    throw new Error('Failed to fetch user data');
  }

  const rawResponse: LoggedInResponseRaw = await rawResponsePromise.json();
  if (!rawResponse.id) {
    return { isLoggedIn: false };
  }

  const loggedInDataRaw = rawResponse as LoggedInDataRaw;

  return {
    isLoggedIn: true,
    email: loggedInDataRaw.email,
    userId: loggedInDataRaw.id,
    selectedLeagueId: loggedInDataRaw.selected_league_id,
  };
}
