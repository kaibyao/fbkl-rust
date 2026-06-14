# The three Lambda functions (96e.3/.4/.5) + their shared execution role and the
# API Function URL.
#
# Artifacts come from cargo-lambda:
#   cargo lambda build --release --arm64 --output-format zip
# which writes target/lambda/<bin>/bootstrap.zip per [[bin]] in lambdas/Cargo.toml.
# Build BEFORE `tofu apply` — the zip files must exist. `source_code_hash` is
# guarded with fileexists() so `tofu validate` stays green pre-build.

locals {
  lambda_build_dir = "${path.module}/../target/lambda"
  api_zip          = "${local.lambda_build_dir}/fbkl-api/bootstrap.zip"
  scheduler_zip    = "${local.lambda_build_dir}/fbkl-scheduler/bootstrap.zip"
  session_gc_zip   = "${local.lambda_build_dir}/fbkl-session-gc/bootstrap.zip"
}

# --- Execution role (CloudWatch Logs only; DB access is over the network) ------

data "aws_iam_policy_document" "lambda_assume" {
  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "lambda_exec" {
  name               = "fbkl-lambda-exec"
  description        = "Execution role for all FBKL Lambdas."
  assume_role_policy = data.aws_iam_policy_document.lambda_assume.json
}

resource "aws_iam_role_policy_attachment" "lambda_logs" {
  role       = aws_iam_role.lambda_exec.name
  policy_arn = "arn:${data.aws_partition.current.partition}:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

# --- Shared config ------------------------------------------------------------

locals {
  lambda_runtime      = "provided.al2023"
  lambda_architecture = "arm64"

  # FBKL_DATABASE_URL must be the POOLED Neon endpoint for every Lambda.
  worker_env = {
    FBKL_DATABASE_URL = neon_project.fbkl.connection_uri_pooler
  }
  api_env = {
    FBKL_DATABASE_URL = neon_project.fbkl.connection_uri_pooler
    SESSION_SECRET    = random_password.session_secret.result
  }
}

# --- fbkl-api (96e.3) ---------------------------------------------------------

resource "aws_lambda_function" "api" {
  function_name = "fbkl-api"
  role          = aws_iam_role.lambda_exec.arn
  runtime       = local.lambda_runtime
  architectures = [local.lambda_architecture]
  handler       = "bootstrap"

  filename         = local.api_zip
  source_code_hash = fileexists(local.api_zip) ? filebase64sha256(local.api_zip) : null

  memory_size = 512
  timeout     = 30

  reserved_concurrent_executions = var.api_reserved_concurrency

  environment {
    variables = local.api_env
  }
}

# Function URL (HTTPS, free indefinitely vs API Gateway's 12-month free tier).
# CORS allows exactly the SPA origin WITH credentials so session cookies flow.
resource "aws_lambda_function_url" "api" {
  function_name      = aws_lambda_function.api.function_name
  authorization_type = "NONE"

  cors {
    allow_credentials = true
    allow_origins     = [var.app_origin]
    allow_methods     = ["GET", "POST", "OPTIONS"]
    allow_headers     = ["content-type", "authorization"]
    max_age           = 86400
  }
}

# --- fbkl-scheduler (96e.4) ---------------------------------------------------

resource "aws_lambda_function" "scheduler" {
  function_name = "fbkl-scheduler"
  role          = aws_iam_role.lambda_exec.arn
  runtime       = local.lambda_runtime
  architectures = [local.lambda_architecture]
  handler       = "bootstrap"

  filename         = local.scheduler_zip
  source_code_hash = fileexists(local.scheduler_zip) ? filebase64sha256(local.scheduler_zip) : null

  memory_size = 256
  timeout     = 120

  reserved_concurrent_executions = var.worker_reserved_concurrency

  environment {
    variables = local.worker_env
  }
}

# --- fbkl-session-gc (96e.5) --------------------------------------------------

resource "aws_lambda_function" "session_gc" {
  function_name = "fbkl-session-gc"
  role          = aws_iam_role.lambda_exec.arn
  runtime       = local.lambda_runtime
  architectures = [local.lambda_architecture]
  handler       = "bootstrap"

  filename         = local.session_gc_zip
  source_code_hash = fileexists(local.session_gc_zip) ? filebase64sha256(local.session_gc_zip) : null

  memory_size = 256
  timeout     = 60

  reserved_concurrent_executions = var.worker_reserved_concurrency

  environment {
    variables = local.worker_env
  }
}

output "api_function_url" {
  value       = aws_lambda_function_url.api.function_url
  description = "HTTPS endpoint for the API Lambda — point the SPA's GraphQL client here."
}
