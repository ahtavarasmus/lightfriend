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

variable "domain" {
  description = "Domain name for DNS records"
  type        = string
}

variable "subdomain" {
  description = "Subdomain prefix (e.g. 'enclave' for enclave.example.com)"
  type        = string
}

variable "github_repo" {
  description = "GitHub repository (owner/repo) for OIDC deploy"
  type        = string
}
