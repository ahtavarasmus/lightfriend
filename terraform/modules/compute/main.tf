# Compute module: EC2 instance with Nitro Enclave support
# Creates the enclave host instance with cloudflared

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
    cloudflare_tunnel_token = var.cloudflare_tunnel_token
    environment             = var.environment
    domain                  = var.domain
  }))

  tags = {
    Name = "${var.project_name}-${var.environment}-enclave"
  }

  lifecycle {
    ignore_changes = [ami] # Don't recreate on AMI updates
  }
}
