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

output "app_url" {
  description = "Application URL"
  value       = "https://api.${var.cloudflare_domain}"
}
