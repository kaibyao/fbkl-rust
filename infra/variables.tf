variable "aws_region" {
  type        = string
  default     = "us-east-1"
  description = "AWS region for all resources (Lambda free tier applies in any region)."
}

variable "aws_profile" {
  type        = string
  default     = "fbkl"
  description = "Local AWS CLI/SSO profile to authenticate with. MUST be the personal fbkl account, not the machine default."
}

variable "github_repo" {
  type        = string
  default     = "kaibyao/fbkl-rust"
  description = "owner/repo allowed to assume the CI deploy role via GitHub OIDC."
}

variable "github_deploy_ref" {
  type        = string
  default     = "refs/heads/main"
  description = "Git ref permitted to assume the deploy role. Restricts CI deploys to this branch."
}
