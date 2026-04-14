output "github_deploy_role_arn" {
  description = "IAM role ARN for GitHub Actions OIDC deploy"
  value       = aws_iam_role.github_deploy.arn
}

output "backup_bucket_name" {
  description = "S3 bucket name for encrypted backups"
  value       = aws_s3_bucket.backups.bucket
}

output "launch_template_id" {
  description = "Launch template ID for blue-green deploys"
  value       = aws_launch_template.enclave.id
}
