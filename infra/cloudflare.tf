# Cloudflare Pages projects for the two Vite frontends (96e.8).
#
# Terraform creates the projects only; the actual build + upload happens in CI
# via wrangler (96e.10). Using the free *.pages.dev subdomains for now — the
# logged-in app's subdomain is wired straight into the API Function URL CORS, so
# there's no placeholder origin to keep in sync.
#
# Authenticated via CLOUDFLARE_API_TOKEN (Account · Cloudflare Pages · Edit).

provider "cloudflare" {
  # api_token read from CLOUDFLARE_API_TOKEN
}

resource "cloudflare_pages_project" "app" {
  account_id        = var.cloudflare_account_id
  name              = "fbkl-app"
  production_branch = "main"
}

resource "cloudflare_pages_project" "public" {
  account_id        = var.cloudflare_account_id
  name              = "fbkl-public"
  production_branch = "main"
}

output "app_pages_url" {
  value       = "https://${cloudflare_pages_project.app.subdomain}"
  description = "Logged-in SPA production URL. Set as the GraphQL client's API host origin + used for API CORS."
}

output "public_pages_url" {
  value       = "https://${cloudflare_pages_project.public.subdomain}"
  description = "Public site production URL."
}
