// Reverse-proxies /api/* to the API Lambda Function URL so the browser only ever
// talks to this Pages origin: first-party cookies, no CORS. API_URL is set as a
// Pages env var by Terraform (cloudflare.tf) to the Lambda Function URL.
//
// Only matches /api/* (file lives under functions/api/); the path is preserved
// so the Lambda router sees the same route (e.g. /api/gql).
export async function onRequest(context: {
  request: Request;
  env: { API_URL: string };
}): Promise<Response> {
  const { request, env } = context;

  if (!env.API_URL) {
    return new Response('API_URL is not configured', { status: 500 });
  }

  const incoming = new URL(request.url);
  const target = new URL(env.API_URL);
  target.pathname = incoming.pathname;
  target.search = incoming.search;

  return fetch(new Request(target.toString(), request));
}
