output "instance_id" {
  description = "EC2 instance ID"
  value       = aws_instance.enclave.id
}

output "public_ip" {
  description = "EC2 instance public IP"
  value       = aws_instance.enclave.public_ip
}

output "private_ip" {
  description = "EC2 instance private IP"
  value       = aws_instance.enclave.private_ip
}

output "github_deploy_role_arn" {
  description = "IAM role ARN for GitHub Actions OIDC deploy"
  value       = aws_iam_role.github_deploy.arn
}

output "backup_bucket_name" {
  description = "S3 bucket name for encrypted backups"
  value       = aws_s3_bucket.backups.bucket
}
