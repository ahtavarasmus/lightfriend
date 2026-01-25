variable "environment" {
  description = "Environment name"
  type        = string
}

variable "project_name" {
  description = "Project name for resource naming"
  type        = string
}

variable "instance_type" {
  description = "EC2 instance type"
  type        = string
}

variable "vpc_id" {
  description = "VPC ID"
  type        = string
}

variable "public_subnet_id" {
  description = "Subnet ID for the instance"
  type        = string
}

variable "security_group_id" {
  description = "Security group ID"
  type        = string
}

variable "cloudflare_tunnel_token" {
  description = "Cloudflare tunnel token for cloudflared"
  type        = string
  sensitive   = true
}

variable "domain" {
  description = "Domain name for DNS records"
  type        = string
}
