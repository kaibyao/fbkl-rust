'use server';

import { ACCESS_TOKEN_NAME } from '@/app/_lib/constants';
import { cookies } from 'next/headers';

export default async function authFetch(
  input: string | Request | URL,
  init?: RequestInit,
): Promise<Response> {
  const token = (await cookies()).get(ACCESS_TOKEN_NAME)?.value;
  if (!token) throw new Error('No token found');

  if (!init) {
    init = {};
  }

  if (!init.headers) {
    init.headers = {};
  }

  init.headers = {
    ...init.headers,
    'Ghost-App-Type': 'marketplace',
    Cookie: `${ACCESS_TOKEN_NAME}=${token}`,
  };

  return fetch(input, init);
}
