#!/bin/bash
# Don't use set -e to prevent one failure from stopping the entire script
# We'll check critical services individually

# Lightfriend Enclave Host Setup Script
# Environment: ${environment}
# Domain: ${domain}

echo "Starting Lightfriend enclave host setup..."
echo "Environment: ${environment}"
echo "Domain: ${domain}"

# Update system
echo "Updating system packages..."
dnf update -y

# Install Nitro Enclaves CLI (optional - don't fail if this doesn't work)
echo "Installing Nitro Enclaves CLI..."
if dnf install -y aws-nitro-enclaves-cli aws-nitro-enclaves-cli-devel; then
    echo "Nitro Enclaves CLI installed successfully"

    # Configure Nitro Enclaves allocator
    # Reserve ~8GB RAM and 4 vCPUs for enclave (50% of c6a.2xlarge resources)
    # CPU count must be multiple of 2 (threads per core)
    # c6a.2xlarge has 8 vCPUs/16GB RAM
    # Normal: 4 vCPU to enclave (heavy workload), 2 to parent (routing), 2 buffer
    # Updates: Each enclave gets 2 vCPU temporarily
    mkdir -p /etc/nitro_enclaves
    cat > /etc/nitro_enclaves/allocator.yaml <<EOF
---
memory_mib: 8192
cpu_count: 4
EOF

    # Start Nitro Enclaves allocator
    systemctl enable nitro-enclaves-allocator || echo "Failed to enable nitro-enclaves-allocator"
    systemctl start nitro-enclaves-allocator || echo "Failed to start nitro-enclaves-allocator"

    # Add ec2-user to ne group for enclave management
    usermod -aG ne ec2-user || echo "Failed to add ec2-user to ne group"
else
    echo "WARNING: Nitro Enclaves CLI installation failed - continuing without enclave support"
fi

# Install cloudflared (CRITICAL - must succeed)
echo "Installing cloudflared..."
curl -L --output /tmp/cloudflared.rpm https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-x86_64.rpm
if ! rpm -i /tmp/cloudflared.rpm; then
    echo "ERROR: Failed to install cloudflared"
    exit 1
fi

# Configure cloudflared tunnel
echo "Configuring cloudflared tunnel..."
mkdir -p /etc/cloudflared
cat > /etc/cloudflared/config.yml <<EOF
tunnel: lightfriend-${environment}
credentials-file: /etc/cloudflared/credentials.json
ingress:
  - hostname: api-${environment}.${domain}
    service: http://localhost:3000
  - service: http_status:404
EOF

# Install tunnel using token (creates credentials automatically)
echo "Installing cloudflared service with tunnel token..."
if ! cloudflared service install ${cloudflare_tunnel_token}; then
    echo "ERROR: Failed to install cloudflared service"
    exit 1
fi

# Enable and start cloudflared
echo "Starting cloudflared service..."
systemctl enable cloudflared
if ! systemctl start cloudflared; then
    echo "ERROR: Failed to start cloudflared"
    systemctl status cloudflared
    journalctl -u cloudflared -n 50 --no-pager
    exit 1
fi

# Verify cloudflared is running
sleep 5
if systemctl is-active --quiet cloudflared; then
    echo "cloudflared is running successfully"
else
    echo "WARNING: cloudflared is not running"
    systemctl status cloudflared
fi

# Install Docker for enclave builds
echo "Installing Docker..."
dnf install -y docker
systemctl enable docker
systemctl start docker
usermod -aG docker ec2-user

echo "Lightfriend enclave host setup complete!"
echo "Cloudflared status:"
systemctl status cloudflared --no-pager
