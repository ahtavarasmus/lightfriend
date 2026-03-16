# Outputs from Lightfriend infrastructure

output "vpc_id" {
  description = "VPC ID"
  value       = module.networking.vpc_id
}

output "instance_id" {
  description = "EC2 instance ID"
  value       = module.compute.instance_id
}

output "instance_public_ip" {
  description = "EC2 instance public IP (for debugging, traffic goes through tunnel)"
  value       = module.compute.public_ip
}

output "tunnel_id" {
  description = "Cloudflare tunnel ID"
  value       = module.cloudflare.tunnel_id
}

output "url" {
  description = "Application URL"
  value       = module.cloudflare.url
}

output "github_deploy_role_arn" {
  description = "IAM role ARN for GitHub Actions OIDC deploy"
  value       = module.compute.github_deploy_role_arn
}

output "tunnel_token" {
  description = "Cloudflare tunnel token (add to .env as CLOUDFLARE_TUNNEL_TOKEN)"
  value       = module.cloudflare.tunnel_token
  sensitive   = true
}

output "backup_bucket_name" {
  description = "S3 bucket for encrypted backups (add to .env as S3_BACKUP_BUCKET)"
  value       = module.compute.backup_bucket_name
}
