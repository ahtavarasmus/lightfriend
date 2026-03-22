# Input variables for Lightfriend infrastructure
# Set these via terraform.tfvars, environment variables, or Terraform Cloud

variable "environment" {
  description = "Environment name (e.g., dev-eddie, staging, prod)"
  type        = string
}

variable "project_name" {
  description = "Project name for resource tagging"
  type        = string
  default     = "lightfriend"
}

# AWS Configuration
variable "aws_region" {
  description = "AWS region for resources"
  type        = string
  default     = "us-east-1"
}

variable "vpc_cidr" {
  description = "CIDR block for VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "instance_type" {
  description = "EC2 instance type (must support Nitro Enclaves)"
  type        = string
  default     = "c6a.2xlarge"

  validation {
    condition     = can(regex("^c6a\\.", var.instance_type))
    error_message = "Instance type must be c6a family for Nitro Enclave support."
  }
}

# Cloudflare Configuration
variable "cloudflare_account_id" {
  description = "Cloudflare account ID"
  type        = string
  sensitive   = true
}

variable "cloudflare_zone_id" {
  description = "Cloudflare zone ID for DNS"
  type        = string
  sensitive   = true
}

variable "cloudflare_domain" {
  description = "Domain name for the application"
  type        = string
}

variable "subdomain" {
  description = "Subdomain prefix (e.g. 'enclave' for enclave.example.com)"
  type        = string
  default     = "enclave"
}

variable "github_repo" {
  description = "GitHub repository (owner/repo) for OIDC deploy"
  type        = string
  default     = "ahtavarasmus/lightfriend"
}
