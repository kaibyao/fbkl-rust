# GitHub Actions -> AWS via OIDC (no long-lived access keys in CI).
#
# GitHub's token endpoint acts as an OIDC identity provider. The deploy role
# below trusts ONLY this repo on the configured branch, so a workflow run can
# assume it and get short-lived credentials at deploy time. Nothing secret is
# stored in GitHub. Wires up the AWS half of 96e.10.

data "aws_caller_identity" "current" {}
data "aws_partition" "current" {}

resource "aws_iam_openid_connect_provider" "github" {
  url             = "https://token.actions.githubusercontent.com"
  client_id_list  = ["sts.amazonaws.com"]
  # GitHub's OIDC thumbprints. AWS validates GitHub's cert via its trust store,
  # but the resource still requires this list.
  thumbprint_list = [
    "6938fd4d98bab03faadb97b34396831e3780aea1",
    "1c58a3a8518e8759bf075b76b750d4f2df264fcd",
  ]
}

data "aws_iam_policy_document" "github_assume_role" {
  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRoleWithWebIdentity"]

    principals {
      type        = "Federated"
      identifiers = [aws_iam_openid_connect_provider.github.arn]
    }

    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:aud"
      values   = ["sts.amazonaws.com"]
    }

    # Restrict to this repo + ref so no other repo/branch can assume the role.
    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:sub"
      values   = ["repo:${var.github_repo}:ref:${var.github_deploy_ref}"]
    }
  }
}

resource "aws_iam_role" "github_deploy" {
  name               = "fbkl-github-deploy"
  description        = "Assumed by GitHub Actions (OIDC) to deploy FBKL Lambdas + schedules."
  assume_role_policy = data.aws_iam_policy_document.github_assume_role.json
}

# Least-privilege deploy permissions. Scoped to fbkl-* resources where the AWS
# API supports resource-level scoping.
data "aws_iam_policy_document" "github_deploy" {
  # Update/configure the three Lambda functions + their Function URLs.
  statement {
    sid    = "LambdaDeploy"
    effect = "Allow"
    actions = [
      "lambda:GetFunction",
      "lambda:UpdateFunctionCode",
      "lambda:UpdateFunctionConfiguration",
      "lambda:PublishVersion",
      "lambda:CreateFunctionUrlConfig",
      "lambda:UpdateFunctionUrlConfig",
      "lambda:GetFunctionUrlConfig",
      "lambda:PutFunctionConcurrency",
    ]
    resources = [
      "arn:${data.aws_partition.current.partition}:lambda:${var.aws_region}:${data.aws_caller_identity.current.account_id}:function:fbkl-*",
    ]
  }

  # Pass only the Lambda execution role(s) to the functions.
  statement {
    sid       = "PassExecutionRole"
    effect    = "Allow"
    actions   = ["iam:PassRole"]
    resources = ["arn:${data.aws_partition.current.partition}:iam::${data.aws_caller_identity.current.account_id}:role/fbkl-*"]
    condition {
      test     = "StringEquals"
      variable = "iam:PassedToService"
      values   = ["lambda.amazonaws.com"]
    }
  }

  # Read/write the Terraform state bucket so CI can run terraform.
  statement {
    sid    = "TerraformState"
    effect = "Allow"
    actions = [
      "s3:GetObject",
      "s3:PutObject",
      "s3:ListBucket",
    ]
    resources = [
      "arn:${data.aws_partition.current.partition}:s3:::fbkl-tfstate-${data.aws_caller_identity.current.account_id}",
      "arn:${data.aws_partition.current.partition}:s3:::fbkl-tfstate-${data.aws_caller_identity.current.account_id}/*",
    ]
  }
}

resource "aws_iam_role_policy" "github_deploy" {
  name   = "fbkl-github-deploy"
  role   = aws_iam_role.github_deploy.id
  policy = data.aws_iam_policy_document.github_deploy.json
}

output "github_deploy_role_arn" {
  value       = aws_iam_role.github_deploy.arn
  description = "Set as the role-to-assume in the GitHub Actions deploy workflow (96e.10)."
}
