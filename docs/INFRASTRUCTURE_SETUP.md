# Infrastructure Setup Guide

This guide walks contributors through setting up their own Lightfriend infrastructure environment. Each contributor can have their own isolated environment via Terraform workspaces.

See [INFRASTRUCTURE_PLAN.md](./INFRASTRUCTURE_PLAN.md) for architecture details and design decisions.

## Prerequisites

- AWS account with billing enabled
- Cloudflare account with Zero Trust enabled (free tier works)
- Terraform Cloud account (free tier works)
- Terraform CLI installed locally (v1.5+)
- AWS CLI installed and configured

## 1. Terraform Cloud Setup

Terraform Cloud provides shared state management so multiple contributors can work on infrastructure without conflicts.

### 1.1 Create Organization

1. Sign up at [app.terraform.io](https://app.terraform.io)
2. Create a new organization (e.g., `lightfriend`)
3. Note your organization name for later

### 1.2 Create Workspaces

Create workspaces for each environment. We use the naming convention `lightfriend-<env>`:

| Workspace | Purpose |
|-----------|---------|
| `lightfriend-dev-<yourname>` | Personal development/testing |
| `lightfriend-staging` | Shared staging environment |
| `lightfriend-prod` | Production (restricted access) |

For each workspace:
1. Go to Workspaces → New Workspace
2. Choose "CLI-driven workflow"
3. Name it according to the convention above
4. Set Execution Mode to "Remote" (or "Local" if you prefer local plans)

### 1.3 Generate API Token

1. Go to User Settings → Tokens
2. Create an API token
3. Save this as `TF_CLOUD_TOKEN` in your `.env` file

### 1.4 Configure Workspace Variables

In each workspace, add these variable sets (Settings → Variable Sets):

**AWS Credentials** (mark as sensitive):
- `AWS_ACCESS_KEY_ID`
- `AWS_SECRET_ACCESS_KEY`

**Cloudflare Credentials** (mark as sensitive):
- `CLOUDFLARE_API_TOKEN`
- `CLOUDFLARE_ACCOUNT_ID`

## 2. AWS Setup

### 2.1 Create IAM User for Terraform

1. Go to IAM → Users → Create User
2. Name: `terraform-lightfriend`
3. Attach policies (or create a custom policy with these permissions):

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "ec2:*",
        "vpc:*",
        "iam:CreateRole",
        "iam:DeleteRole",
        "iam:AttachRolePolicy",
        "iam:DetachRolePolicy",
        "iam:CreateInstanceProfile",
        "iam:DeleteInstanceProfile",
        "iam:AddRoleToInstanceProfile",
        "iam:RemoveRoleFromInstanceProfile",
        "iam:GetRole",
        "iam:GetInstanceProfile",
        "iam:PassRole",
        "iam:ListRolePolicies",
        "iam:ListAttachedRolePolicies",
        "iam:ListInstanceProfilesForRole"
      ],
      "Resource": "*"
    }
  ]
}
```

4. Create access keys (Security Credentials → Create Access Key)
5. Save `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`

### 2.2 Choose Region

We recommend `us-east-1` for Nitro Enclave support. Ensure your chosen region supports:
- Nitro Enclaves (c6a instances)
- Required instance types (c6a.2xlarge minimum)

Set `AWS_REGION` in your `.env` file.

### 2.3 Verify Nitro Enclave Availability

```bash
aws ec2 describe-instance-types \
  --instance-types c6a.2xlarge \
  --query "InstanceTypes[].EnclaveOptions.Supported"
```

Should return `[true]`.

## 3. Cloudflare Setup

### 3.1 Enable Zero Trust

1. Log in to Cloudflare Dashboard
2. Go to Zero Trust (left sidebar)
3. If prompted, set up your Zero Trust organization

### 3.2 Create API Token

1. Go to My Profile → API Tokens
2. Create Custom Token with these permissions:

| Permission | Access |
|------------|--------|
| Account / Cloudflare Tunnel | Edit |
| Account / Access: Apps and Policies | Edit |
| Zone / DNS | Edit |

3. Save as `CLOUDFLARE_API_TOKEN`

### 3.3 Get Account and Zone IDs

1. Account ID: Found on the right sidebar of any zone's Overview page
2. Zone ID: Found on the Overview page of your domain

Save as `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_ZONE_ID`.

## 4. Local Environment Setup

### 4.1 Configure Environment Variables

Copy the example environment file:

```bash
cp .env.example .env
```

Fill in the infrastructure section:

```bash
# Infrastructure - Terraform Cloud
TF_CLOUD_ORG=lightfriend
TF_CLOUD_TOKEN=<your-terraform-cloud-token>

# Infrastructure - AWS
AWS_ACCESS_KEY_ID=<your-aws-access-key>
AWS_SECRET_ACCESS_KEY=<your-aws-secret-key>
AWS_REGION=us-east-1

# Infrastructure - Cloudflare
CLOUDFLARE_API_TOKEN=<your-cloudflare-token>
CLOUDFLARE_ACCOUNT_ID=<your-account-id>
CLOUDFLARE_ZONE_ID=<your-zone-id>
CLOUDFLARE_DOMAIN=<your-domain.com>
```

### 4.2 Initialize Terraform

```bash
cd terraform

# Login to Terraform Cloud
terraform login

# Initialize with your workspace
terraform init

# Select your workspace
terraform workspace select lightfriend-dev-<yourname>
# Or create it if it doesn't exist:
terraform workspace new lightfriend-dev-<yourname>
```

### 4.3 Plan and Apply

```bash
# Review what will be created
terraform plan

# Apply (creates real resources - costs money!)
terraform apply
```

## 5. Verify Setup

After `terraform apply` completes:

### 5.1 Check EC2 Instance

```bash
aws ec2 describe-instances \
  --filters "Name=tag:Project,Values=lightfriend" \
  --query "Reservations[].Instances[].{ID:InstanceId,State:State.Name,IP:PublicIpAddress}"
```

### 5.2 Check Cloudflare Tunnel

```bash
# Via Cloudflare API
curl -X GET "https://api.cloudflare.com/client/v4/accounts/${CLOUDFLARE_ACCOUNT_ID}/cfd_tunnel" \
  -H "Authorization: Bearer ${CLOUDFLARE_API_TOKEN}" \
  -H "Content-Type: application/json"
```

Or check in Cloudflare Dashboard → Zero Trust → Networks → Tunnels.

### 5.3 Test Connectivity

Once the tunnel is running:

```bash
curl https://api.<your-domain>/health
```

## 6. Teardown

To destroy your development environment:

```bash
cd terraform
terraform workspace select lightfriend-dev-<yourname>
terraform destroy
```

**Warning**: This destroys all resources including data. Only do this for dev environments.

## Troubleshooting

### Terraform state lock

If you see state lock errors, another operation may be in progress. Wait or force unlock:

```bash
terraform force-unlock <lock-id>
```

### Nitro Enclave not starting

Check allocator configuration:

```bash
# On the EC2 instance
cat /etc/nitro_enclaves/allocator.yaml
sudo systemctl status nitro-enclaves-allocator
```

### Cloudflare tunnel not connecting

Check cloudflared service:

```bash
# On the EC2 instance
sudo systemctl status cloudflared
sudo journalctl -u cloudflared -f
```

## Directory Structure

```
terraform/
├── main.tf              # Root module, backend config
├── variables.tf         # Input variables
├── outputs.tf           # Output values
├── providers.tf         # Provider configuration
├── terraform.tfvars     # Variable values (git-ignored)
└── modules/
    ├── networking/      # VPC, subnets, security groups
    ├── compute/         # EC2 instance, Nitro config
    └── cloudflare/      # Tunnel, DNS, access policies
```

See each module's README for detailed configuration options.
