'use server';

import { BASE_API_URL } from '@/app/_lib/constants';
import authFetch from '@/app/_lib/api';

const LOGIN_API_URL = `${BASE_API_URL}/api/login`;

export async function processLogin({
  email,
  password,
}: {
  email: string;
  password: string;
}) {
  const response = await authFetch(LOGIN_API_URL, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ email, password }),
  });

  if (response.status === 200) {
    return true;
  }

  return false;
}
