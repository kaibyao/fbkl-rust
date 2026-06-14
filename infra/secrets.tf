# Session secret for the API Lambda's tower-sessions private-cookie key.
#
# Generated once and stored in state (encrypted in S3), so it stays STABLE across
# deploys — rotating it would invalidate every existing login. The app derives a
# fixed 64-byte key from this via SHA-512, so any length is fine.
resource "random_password" "session_secret" {
  length  = 48
  special = false
}
