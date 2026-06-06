import authFetch from '@/lib/api';
import { BASE_API_URL } from '@/lib/constants';

interface LoggedInDataRaw {
  id: number;
  email: string;
  selected_league_id: number;
  selected_league_owner_team_id: number;
}

type NotLoggedInResponse = { id: never };

type LoggedInResponseRaw = LoggedInDataRaw | NotLoggedInResponse;

export interface LoggedIn {
  isLoggedIn: true;
  email: string;
  userId: number;
  selectedLeagueId: number;
  selectedLeagueOwnerTeamId?: number;
}

interface NotLoggedIn {
  isLoggedIn: false;
}

export type LoggedInResponse = LoggedIn | NotLoggedIn;

const GET_USER_DATA_API_URL = `${BASE_API_URL}/api/user`;
const LOGIN_API_URL = `${BASE_API_URL}/api/login`;

export async function getUserData(): Promise<LoggedInResponse> {
  let response: Response;
  try {
    response = await authFetch(GET_USER_DATA_API_URL, {
      headers: {
        'Content-Type': 'application/json',
      },
    });
  } catch {
    // Network error (e.g. backend unreachable) — treat as logged-out.
    return { isLoggedIn: false };
  }

  // Treat any non-200 (e.g. expired/missing cookie) as logged-out so route
  // guards redirect to /login rather than throwing.
  if (response.status !== 200) {
    return { isLoggedIn: false };
  }

  const rawResponse: LoggedInResponseRaw = await response.json();
  if (!rawResponse.id) {
    return { isLoggedIn: false };
  }

  const loggedInDataRaw = rawResponse as LoggedInDataRaw;

  return {
    isLoggedIn: true,
    email: loggedInDataRaw.email,
    userId: loggedInDataRaw.id,
    selectedLeagueId: loggedInDataRaw.selected_league_id,
    selectedLeagueOwnerTeamId: loggedInDataRaw.selected_league_owner_team_id,
  };
}

export async function processLogin({
  email,
  password,
}: {
  email: string;
  password: string;
}): Promise<boolean> {
  const response = await authFetch(LOGIN_API_URL, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ email, password }),
  });

  return response.status === 200;
}
