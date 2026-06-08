// Client-side authenticated fetch. The browser sends the `fbkl_id` cookie
// automatically for same-origin requests, so no manual token handling is
// needed (unlike the previous Next.js server actions).
export default async function authFetch(
  input: string | Request | URL,
  init: RequestInit = {},
): Promise<Response> {
  return fetch(input, {
    ...init,
    credentials: 'include',
    headers: {
      ...init.headers,
      'Ghost-App-Type': 'marketplace',
    },
  });
}
