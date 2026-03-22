# Compute module: EC2 instance with Nitro Enclave support
# Creates the enclave host instance, VSOCK services, and S3 backup bucket

data "aws_ami" "amazon_linux_2023" {
  most_recent = true
  owners      = ["amazon"]

  filter {
    name   = "name"
    values = ["al2023-ami-*-x86_64"]
  }

  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }
}

resource "aws_iam_role" "enclave" {
  name = "${var.project_name}-${var.environment}-enclave-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "ec2.amazonaws.com"
        }
      }
    ]
  })
}

resource "aws_iam_instance_profile" "enclave" {
  name = "${var.project_name}-${var.environment}-enclave-profile"
  role = aws_iam_role.enclave.name
}

# SSM for remote management (optional but recommended)
resource "aws_iam_role_policy_attachment" "ssm" {
  role       = aws_iam_role.enclave.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

# Launch template - used by both initial terraform apply and CI blue-green deploys
resource "aws_launch_template" "enclave" {
  name = "${var.project_name}-${var.environment}-enclave"

  image_id      = data.aws_ami.amazon_linux_2023.id
  instance_type = var.instance_type

  iam_instance_profile {
    name = aws_iam_instance_profile.enclave.name
  }

  enclave_options {
    enabled = true
  }

  block_device_mappings {
    device_name = "/dev/xvda"
    ebs {
      volume_size = 50
      volume_type = "gp3"
      encrypted   = true
    }
  }

  vpc_security_group_ids = [var.security_group_id]

  user_data = base64encode(templatefile("${path.module}/user_data.sh", {
    environment = var.environment
    domain      = var.domain
    subdomain   = var.subdomain
  }))

  tag_specifications {
    resource_type = "instance"
    tags = {
      Name = "${var.project_name}-${var.environment}-enclave"
    }
  }

  lifecycle {
    ignore_changes = [image_id]
  }
}

# Initial EC2 instance - created by terraform, subsequent instances created by CI
resource "aws_instance" "enclave" {
  subnet_id = var.public_subnet_id

  launch_template {
    id      = aws_launch_template.enclave.id
    version = "$Latest"
  }

  tags = {
    Name = "${var.project_name}-${var.environment}-enclave"
  }

  lifecycle {
    ignore_changes = [ami, launch_template] # CI manages subsequent instances
  }
}

# ── S3 bucket for encrypted backups ──────────────────────────────────────────

resource "aws_s3_bucket" "backups" {
  bucket = "${var.project_name}-${var.environment}-backups"
}

resource "aws_s3_bucket_server_side_encryption_configuration" "backups" {
  bucket = aws_s3_bucket.backups.id
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

# Keep old backup versions recoverable even if overwritten
resource "aws_s3_bucket_public_access_block" "backups" {
  bucket = aws_s3_bucket.backups.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_versioning" "backups" {
  bucket = aws_s3_bucket.backups.id
  versioning_configuration {
    status = "Enabled"
  }
}

# Automatic cleanup of old backups and deploy artifacts
resource "aws_s3_bucket_lifecycle_configuration" "backups" {
  bucket = aws_s3_bucket.backups.id

  rule {
    id     = "retain-hourly-backups-72-hours"
    status = "Enabled"
    filter {
      prefix = "backups/hourly/"
    }
    expiration {
      days = 3
    }
    noncurrent_version_expiration {
      noncurrent_days = 7
    }
  }

  rule {
    id     = "retain-daily-backups-35-days"
    status = "Enabled"
    filter {
      prefix = "backups/daily/"
    }
    expiration {
      days = 35
    }
    noncurrent_version_expiration {
      noncurrent_days = 7
    }
  }

  rule {
    id     = "retain-weekly-backups-84-days"
    status = "Enabled"
    filter {
      prefix = "backups/weekly/"
    }
    expiration {
      days = 84
    }
    noncurrent_version_expiration {
      noncurrent_days = 7
    }
  }

  rule {
    id     = "retain-monthly-backups-365-days"
    status = "Enabled"
    filter {
      prefix = "backups/monthly/"
    }
    expiration {
      days = 365
    }
    noncurrent_version_expiration {
      noncurrent_days = 7
    }
  }

  rule {
    id     = "retain-deploy-logs-365-days"
    status = "Enabled"
    filter {
      prefix = "deploy/logs/"
    }
    expiration {
      days = 365
    }
  }

  rule {
    id     = "cleanup-deploy-artifacts-7-days"
    status = "Enabled"
    filter {
      prefix = "deploy/verify-"
    }
    expiration {
      days = 7
    }
  }
}

resource "aws_iam_role_policy" "enclave_s3" {
  name = "s3-backup-access"
  role = aws_iam_role.enclave.id
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = ["s3:GetObject", "s3:PutObject", "s3:DeleteObject", "s3:ListBucket"]
      Resource = [
        aws_s3_bucket.backups.arn,
        "${aws_s3_bucket.backups.arn}/*"
      ]
    }]
  })
}

resource "aws_iam_role_policy" "enclave_ssm_parameters" {
  name = "ssm-parameter-read-access"
  role = aws_iam_role.enclave.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = ["ssm:GetParameter"]
      Resource = "arn:aws:ssm:*:*:parameter/lightfriend/*"
    }]
  })
}

# ── GitHub Actions OIDC deploy ───────────────────────────────────────────────

# If this provider already exists in the AWS account, import it:
#   terraform import module.compute.aws_iam_openid_connect_provider.github arn:aws:iam::ACCOUNT:oidc-provider/token.actions.githubusercontent.com
resource "aws_iam_openid_connect_provider" "github" {
  url             = "https://token.actions.githubusercontent.com"
  client_id_list  = ["sts.amazonaws.com"]
  thumbprint_list = ["6938fd4d98bab03faadb97b34396831e3780aea1"]
}

resource "aws_iam_role" "github_deploy" {
  name = "${var.project_name}-${var.environment}-github-deploy"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Principal = {
          Federated = aws_iam_openid_connect_provider.github.arn
        }
        Action = "sts:AssumeRoleWithWebIdentity"
        Condition = {
          StringEquals = {
            "token.actions.githubusercontent.com:aud" = "sts.amazonaws.com"
          }
          StringLike = {
            "token.actions.githubusercontent.com:sub" = "repo:${var.github_repo}:ref:refs/heads/*"
          }
        }
      }
    ]
  })
}

resource "aws_iam_role_policy" "github_deploy_ssm" {
  name = "ssm-send-command"
  role = aws_iam_role.github_deploy.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "ssm:SendCommand",
          "ssm:GetCommandInvocation"
        ]
        Resource = [
          "arn:aws:ec2:*:*:instance/*",
          "arn:aws:ssm:*:*:document/AWS-RunShellScript"
        ]
      }
    ]
  })
}

resource "aws_iam_role_policy" "github_deploy_s3" {
  name = "s3-backup-access"
  role = aws_iam_role.github_deploy.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = ["s3:GetObject", "s3:PutObject", "s3:DeleteObject", "s3:ListBucket"]
      Resource = [
        aws_s3_bucket.backups.arn,
        "${aws_s3_bucket.backups.arn}/*"
      ]
    }]
  })
}

# EC2 permissions for blue-green deploy (create new instance, terminate old)
resource "aws_iam_role_policy" "github_deploy_ec2" {
  name = "ec2-blue-green"
  role = aws_iam_role.github_deploy.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "ec2:RunInstances",
          "ec2:TerminateInstances",
          "ec2:DescribeInstances",
          "ec2:DescribeInstanceStatus",
          "ec2:CreateTags",
          "ec2:DescribeLaunchTemplateVersions"
        ]
        Resource = "*"
      },
      {
        Effect = "Allow"
        Action = "iam:PassRole"
        Resource = aws_iam_role.enclave.arn
      },
      {
        Effect = "Allow"
        Action = [
          "ssm:GetParameter",
          "ssm:PutParameter"
        ]
        Resource = "arn:aws:ssm:*:*:parameter/lightfriend/*"
      }
    ]
  })
}

# ── SSM Parameters for blue-green deploy ─────────────────────────────────────
# CI reads these to create new EC2 instances with identical config

resource "aws_ssm_parameter" "launch_template_id" {
  name  = "/lightfriend/launch-template-id"
  type  = "String"
  value = aws_launch_template.enclave.id
}

resource "aws_ssm_parameter" "subnet_id" {
  name  = "/lightfriend/subnet-id"
  type  = "String"
  value = var.public_subnet_id
}

resource "aws_ssm_parameter" "s3_bucket" {
  name  = "/lightfriend/s3-bucket"
  type  = "String"
  value = aws_s3_bucket.backups.bucket
}

resource "aws_ssm_parameter" "instance_id" {
  name  = "/lightfriend/instance-id"
  type  = "String"
  value = aws_instance.enclave.id

  lifecycle {
    ignore_changes = [value] # CI updates this during blue-green deploys
  }
}
