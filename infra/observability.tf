# CloudWatch alarms + SNS email alerts for the Lambdas (96e.9).
#
# One Errors alarm and one Throttles alarm per function, all notifying a single
# SNS topic that emails alert_email. (Sentry's Rust + browser SDK integration is
# tracked separately — it needs a DSN and app-code changes.)
#
# NOTE: the email subscription must be confirmed via the link AWS sends before
# alerts deliver.

resource "aws_sns_topic" "alerts" {
  name = "fbkl-alerts"
}

resource "aws_sns_topic_subscription" "alerts_email" {
  topic_arn = aws_sns_topic.alerts.arn
  protocol  = "email"
  endpoint  = var.alert_email
}

locals {
  alarm_functions = {
    api        = aws_lambda_function.api.function_name
    scheduler  = aws_lambda_function.scheduler.function_name
    session_gc = aws_lambda_function.session_gc.function_name
  }
}

resource "aws_cloudwatch_metric_alarm" "errors" {
  for_each = local.alarm_functions

  alarm_name          = "fbkl-${each.key}-errors"
  alarm_description   = "fbkl-${each.value} reported function errors."
  namespace           = "AWS/Lambda"
  metric_name         = "Errors"
  dimensions          = { FunctionName = each.value }
  statistic           = "Sum"
  period              = 300
  evaluation_periods  = 1
  threshold           = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  treat_missing_data  = "notBreaching"

  alarm_actions = [aws_sns_topic.alerts.arn]
  ok_actions    = [aws_sns_topic.alerts.arn]
}

resource "aws_cloudwatch_metric_alarm" "throttles" {
  for_each = local.alarm_functions

  alarm_name          = "fbkl-${each.key}-throttles"
  alarm_description   = "fbkl-${each.value} is being throttled (concurrency cap hit)."
  namespace           = "AWS/Lambda"
  metric_name         = "Throttles"
  dimensions          = { FunctionName = each.value }
  statistic           = "Sum"
  period              = 300
  evaluation_periods  = 1
  threshold           = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  treat_missing_data  = "notBreaching"

  alarm_actions = [aws_sns_topic.alerts.arn]
  ok_actions    = [aws_sns_topic.alerts.arn]
}
