#!/bin/bash
set -e

# Lightfriend Enclave Host Setup Script
# Environment: ${environment}

echo "Starting Lightfriend enclave host setup..."

# Update system
dnf update -y

# Install Nitro Enclaves CLI
amazon-linux-extras install aws-nitro-enclaves-cli -y || dnf install -y aws-nitro-enclaves-cli aws-nitro-enclaves-cli-devel

# Configure Nitro Enclaves allocator
# Reserve ~6GB RAM and 3 vCPUs for enclave (half of c6a.2xlarge)
cat > /etc/nitro_enclaves/allocator.yaml <<EOF
---
memory_mib: 6144
cpu_count: 3
EOF

# Start Nitro Enclaves allocator
systemctl enable nitro-enclaves-allocator
systemctl start nitro-enclaves-allocator

# Add ec2-user to ne group for enclave management
usermod -aG ne ec2-user

# Install cloudflared
curl -L --output /tmp/cloudflared.rpm https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-x86_64.rpm
rpm -i /tmp/cloudflared.rpm

# Configure cloudflared tunnel
mkdir -p /etc/cloudflared
cat > /etc/cloudflared/config.yml <<EOF
tunnel: lightfriend-${environment}
credentials-file: /etc/cloudflared/credentials.json
ingress:
  - hostname: api.${environment}.lightfriend.io
    service: http://localhost:3000
  - service: http_status:404
EOF

# Install tunnel using token (creates credentials automatically)
cloudflared service install ${cloudflare_tunnel_token}

# Enable and start cloudflared
systemctl enable cloudflared
systemctl start cloudflared

# Install Docker for enclave builds
dnf install -y docker
systemctl enable docker
systemctl start docker
usermod -aG docker ec2-user

echo "Lightfriend enclave host setup complete!"
