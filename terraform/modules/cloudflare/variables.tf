variable "environment" {
  description = "Environment name"
  type        = string
}

variable "project_name" {
  description = "Project name for resource naming"
  type        = string
}

variable "cloudflare_account_id" {
  description = "Cloudflare account ID"
  type        = string
}

variable "cloudflare_zone_id" {
  description = "Cloudflare zone ID"
  type        = string
}

variable "domain" {
  description = "Domain name"
  type        = string
}

variable "subdomain" {
  description = "Subdomain prefix (e.g. 'enclave' for enclave.example.com)"
  type        = string
}
