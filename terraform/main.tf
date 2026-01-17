# Lightfriend Infrastructure
# See docs/INFRASTRUCTURE_PLAN.md for architecture details
# See docs/INFRASTRUCTURE_SETUP.md for setup instructions

terraform {
  required_version = ">= 1.5.0"

  cloud {
    organization = "lightfriend" # Update via TF_CLOUD_ORG env var or terraform.tfvars

    workspaces {
      tags = ["lightfriend"]
    }
  }

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 4.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.0"
    }
  }
}

# Networking: VPC, subnets, security groups
module "networking" {
  source = "./modules/networking"

  environment    = var.environment
  vpc_cidr       = var.vpc_cidr
  aws_region     = var.aws_region
  project_name   = var.project_name
}

# Compute: EC2 instance with Nitro Enclave support
module "compute" {
  source = "./modules/compute"

  environment          = var.environment
  instance_type        = var.instance_type
  project_name         = var.project_name
  vpc_id               = module.networking.vpc_id
  public_subnet_id     = module.networking.public_subnet_id
  security_group_id    = module.networking.security_group_id
  cloudflare_tunnel_token = module.cloudflare.tunnel_token
}

# Cloudflare: Zero Trust tunnel and DNS
module "cloudflare" {
  source = "./modules/cloudflare"

  environment         = var.environment
  cloudflare_zone_id  = var.cloudflare_zone_id
  cloudflare_account_id = var.cloudflare_account_id
  domain              = var.cloudflare_domain
  project_name        = var.project_name
}
