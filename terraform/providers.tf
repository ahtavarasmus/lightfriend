# Provider configuration
# Credentials are set via environment variables or Terraform Cloud workspace variables

provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = var.project_name
      Environment = var.environment
      ManagedBy   = "terraform"
    }
  }
}

provider "cloudflare" {
  # API token set via CLOUDFLARE_API_TOKEN environment variable
}

provider "random" {}
