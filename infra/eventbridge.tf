# EventBridge Scheduler (96e.7): replaces the in-process poll loops.
#   - scheduler  : every 1 minute (EventBridge minimum) -> fbkl-scheduler
#   - session-gc : every 5 minutes                       -> fbkl-session-gc
#
# Uses EventBridge *Scheduler* (aws_scheduler_schedule), not legacy CloudWatch
# Events rules: 14M free invocations/mo and a per-schedule invoke role. Double
# fires are safe — the scheduler tick is idempotent via job_run claims, and
# delete_expired is naturally idempotent.

# Role EventBridge Scheduler assumes to invoke the target Lambdas.
data "aws_iam_policy_document" "scheduler_assume" {
  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["scheduler.amazonaws.com"]
    }
    # Guard against the confused-deputy problem: only this account's schedules.
    condition {
      test     = "StringEquals"
      variable = "aws:SourceAccount"
      values   = [data.aws_caller_identity.current.account_id]
    }
  }
}

resource "aws_iam_role" "scheduler_invoke" {
  name               = "fbkl-scheduler-invoke"
  description        = "Assumed by EventBridge Scheduler to invoke FBKL worker Lambdas."
  assume_role_policy = data.aws_iam_policy_document.scheduler_assume.json
}

data "aws_iam_policy_document" "scheduler_invoke" {
  statement {
    effect  = "Allow"
    actions = ["lambda:InvokeFunction"]
    resources = [
      aws_lambda_function.scheduler.arn,
      aws_lambda_function.session_gc.arn,
    ]
  }
}

resource "aws_iam_role_policy" "scheduler_invoke" {
  name   = "fbkl-scheduler-invoke"
  role   = aws_iam_role.scheduler_invoke.id
  policy = data.aws_iam_policy_document.scheduler_invoke.json
}

# --- Schedules ----------------------------------------------------------------

resource "aws_scheduler_schedule" "scheduler_tick" {
  name = "fbkl-scheduler-tick"

  flexible_time_window {
    mode = "OFF"
  }

  schedule_expression          = "rate(1 minute)"
  schedule_expression_timezone = "UTC"

  target {
    arn      = aws_lambda_function.scheduler.arn
    role_arn = aws_iam_role.scheduler_invoke.arn

    retry_policy {
      maximum_retry_attempts = 0 # tick is cheap + re-runs next minute; no retry needed
    }
  }
}

resource "aws_scheduler_schedule" "session_gc" {
  name = "fbkl-session-gc"

  flexible_time_window {
    mode = "OFF"
  }

  schedule_expression          = "rate(5 minutes)"
  schedule_expression_timezone = "UTC"

  target {
    arn      = aws_lambda_function.session_gc.arn
    role_arn = aws_iam_role.scheduler_invoke.arn

    retry_policy {
      maximum_retry_attempts = 0
    }
  }
}
