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

resource "aws_instance" "enclave" {
  ami                    = data.aws_ami.amazon_linux_2023.id
  instance_type          = var.instance_type
  subnet_id              = var.public_subnet_id
  vpc_security_group_ids = [var.security_group_id]
  iam_instance_profile   = aws_iam_instance_profile.enclave.name

  # Enable Nitro Enclaves
  enclave_options {
    enabled = true
  }

  root_block_device {
    volume_size = 50
    volume_type = "gp3"
    encrypted   = true
  }

  user_data = base64encode(templatefile("${path.module}/user_data.sh", {
    environment = var.environment
    domain      = var.domain
    subdomain   = var.subdomain
  }))

  tags = {
    Name = "${var.project_name}-${var.environment}-enclave"
  }

  lifecycle {
    ignore_changes = [ami] # Don't recreate on AMI updates
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

resource "aws_iam_role_policy" "enclave_s3" {
  name = "s3-backup-access"
  role = aws_iam_role.enclave.id
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = ["s3:GetObject", "s3:PutObject", "s3:ListBucket"]
      Resource = [
        aws_s3_bucket.backups.arn,
        "${aws_s3_bucket.backups.arn}/*"
      ]
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
          "arn:aws:ec2:*:*:instance/${aws_instance.enclave.id}",
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
      Action = ["s3:GetObject", "s3:PutObject", "s3:ListBucket"]
      Resource = [
        aws_s3_bucket.backups.arn,
        "${aws_s3_bucket.backups.arn}/*"
      ]
    }]
  })
}
