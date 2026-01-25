# Infrastructure Setup Guide

This guide walks contributors through setting up their own Lightfriend infrastructure environment. Each contributor can have their own isolated environment via Terraform workspaces.

See [INFRASTRUCTURE_PLAN.md](./INFRASTRUCTURE_PLAN.md) for architecture details and design decisions.

## Prerequisites

- AWS account with billing enabled
- Cloudflare account with:
  - A domain already added and active in Cloudflare
  - Zero Trust enabled (free tier works)
- Terraform Cloud account (free tier works)
- Terraform CLI installed locally (v1.5+)
- AWS CLI installed and configured

## 1. Terraform Cloud Setup

Terraform Cloud provides shared state management so multiple contributors can work on infrastructure without conflicts.

### 1.1 Create Organization

1. Sign up at [app.terraform.io](https://app.terraform.io)
2. Create a new organization (e.g., `lightfriend`)
3. Note your organization name for later

**Organization Type Tradeoffs:**

| Type | Cost | Approval Controls | Best For |
|------|------|-------------------|----------|
| **Personal** | Free | None - any team member can apply changes | Personal projects, solo development |
| **Business** | $20/user/month | Policy-as-code, sentinel policies, approval workflows | Team projects requiring governance |

For most contributors, start with a **Personal** organization. Upgrade to Business if you need approval controls for production deployments.

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
5. After creation, go to Settings → General → Tags
6. Add tag: `lightfriend`
7. Save changes

**Note**: The `lightfriend` tag is required for Terraform to find your workspace. Alternatively, you can skip creating workspaces manually and let `terraform init` create them with the correct tag when prompted.

### 1.3 Generate API Token

1. Go to your organization settings → Team API tokens (not User Settings)
2. Create a **Team token** (recommended) or a User token
   - **Team tokens**: Scoped to organization, can be rotated without affecting personal account
   - **User tokens**: Tied to your personal account, have broader access
3. Save this as `TF_CLOUD_TOKEN` in your `.env` file

**Note**: Team tokens are preferred for security and easier rotation. Only use User tokens if you need cross-organization access.

### 1.4 Configure Workspace Variables

In each workspace, configure the following:

#### AWS Dynamic Credentials (OIDC)

1. Go to workspace Settings → Variable sets → Add variable set
2. Select "Create variable set"
3. Name it "AWS Dynamic Credentials"
4. Add these **Environment variables**:
   - `TFC_AWS_PROVIDER_AUTH` = `true`
   - `TFC_AWS_RUN_ROLE_ARN` = `arn:aws:iam::YOUR_AWS_ACCOUNT_ID:role/terraform-lightfriend-role`

   Replace `YOUR_AWS_ACCOUNT_ID` with your AWS account ID.

5. Apply to all workspaces (or specific ones)

**Note**: These special TFC_* variables tell Terraform Cloud to use OIDC authentication instead of static credentials.

#### Cloudflare Credentials (Variable Set - Recommended)

Create a **Variable Set** to share Cloudflare credentials across all workspaces:

1. Go to your organization Settings → Variable sets
2. Create new variable set: "Cloudflare Credentials"
3. Add these variables:
   - `CLOUDFLARE_API_TOKEN` (Environment variable, sensitive) - API token from section 3.3
   - `cloudflare_account_id` (Terraform variable, sensitive) - Your Cloudflare Account ID
   - `cloudflare_zone_id` (Terraform variable, sensitive) - Your Cloudflare Zone ID
   - `cloudflare_domain` (Terraform variable) - Your domain (e.g., `example.com`)
4. Apply to all workspaces with the `lightfriend` tag

**Per-Workspace Variable:**

In each individual workspace, add:
- `environment` (Terraform variable) - e.g., `dev-eddie`, `staging`, `prod`

This setup allows multiple environments to coexist on the same domain using environment-specific subdomains (e.g., `api-dev-eddie.example.com`, `api-staging.example.com`).

## 2. AWS Setup

We use **OIDC dynamic credentials** instead of long-term access keys for better security. Terraform Cloud authenticates directly to AWS using short-lived tokens.

### 2.1 Get Your AWS Account ID

You'll need this for the following steps:
1. Go to AWS Console
2. Click your username in the top-right
3. Copy your 12-digit Account ID

### 2.2 Create OIDC Identity Provider

This allows Terraform Cloud to authenticate to your AWS account.

1. Go to IAM → Identity providers → Add provider
2. Configure:
   - Provider type: **OpenID Connect**
   - Provider URL: `https://app.terraform.io`
   - Audience: `aws.workload.identity`
3. Click "Add provider"

### 2.3 Create IAM Role for Terraform

1. Go to IAM → Roles → Create Role
2. Select trusted entity type: **Web identity**
3. Configure web identity:
   - Identity provider: Select `app.terraform.io` from dropdown (the one you just created)
   - Audience: `aws.workload.identity`
   - Workload type: Select **"Workspace run"** (for standard Terraform Cloud workspaces)
   - Organization: Enter your Terraform Cloud organization name (e.g., `lightfriend`)
   - Project name: Enter `*` (wildcard - allows all projects)
   - Workspace name: Enter `*` (wildcard - allows all workspaces)
   - Run phase: Enter `*` (wildcard - allows all run phases)
4. Click "Next"
5. Skip adding permissions for now (click "Next" again)
6. Name the role: `terraform-lightfriend-role`
7. Click "Create role"

**Note**: Using wildcards (`*`) creates a trust policy that allows ANY workspace in your organization to use this role, which gives contributors flexibility to create their own dev workspaces.

### 2.4 Add Permissions Policy

Now add the permissions policy to the role:

1. Go to IAM → Roles → `terraform-lightfriend-role`
2. Permissions tab → Add permissions → Create inline policy
3. Switch to JSON and paste:

**Important**: Replace `YOUR_AWS_ACCOUNT_ID` with your actual 12-digit AWS account ID.

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "EC2AndVPCManagement",
      "Effect": "Allow",
      "Action": [
        "ec2:RunInstances",
        "ec2:StartInstances",
        "ec2:StopInstances",
        "ec2:TerminateInstances",
        "ec2:DescribeInstances",
        "ec2:DescribeInstanceTypes",
        "ec2:DescribeInstanceAttribute",
        "ec2:ModifyInstanceAttribute",
        "ec2:DescribeImages",
        "ec2:CreateVpc",
        "ec2:DeleteVpc",
        "ec2:DescribeVpcs",
        "ec2:DescribeVpcAttribute",
        "ec2:ModifyVpcAttribute",
        "ec2:CreateSubnet",
        "ec2:DeleteSubnet",
        "ec2:DescribeSubnets",
        "ec2:ModifySubnetAttribute",
        "ec2:CreateInternetGateway",
        "ec2:DeleteInternetGateway",
        "ec2:DescribeInternetGateways",
        "ec2:AttachInternetGateway",
        "ec2:DetachInternetGateway",
        "ec2:CreateRouteTable",
        "ec2:DeleteRouteTable",
        "ec2:DescribeRouteTables",
        "ec2:CreateRoute",
        "ec2:DeleteRoute",
        "ec2:AssociateRouteTable",
        "ec2:DisassociateRouteTable",
        "ec2:CreateSecurityGroup",
        "ec2:DeleteSecurityGroup",
        "ec2:DescribeSecurityGroups",
        "ec2:AuthorizeSecurityGroupIngress",
        "ec2:AuthorizeSecurityGroupEgress",
        "ec2:RevokeSecurityGroupIngress",
        "ec2:RevokeSecurityGroupEgress",
        "ec2:DescribeVolumes",
        "ec2:CreateVolume",
        "ec2:DeleteVolume",
        "ec2:AttachVolume",
        "ec2:DetachVolume",
        "ec2:DescribeNetworkInterfaces",
        "ec2:DeleteNetworkInterface",
        "ec2:CreateTags",
        "ec2:DeleteTags",
        "ec2:DescribeTags"
      ],
      "Resource": "*"
    },
    {
      "Sid": "IAMRoleManagement",
      "Effect": "Allow",
      "Action": [
        "iam:CreateRole",
        "iam:DeleteRole",
        "iam:GetRole",
        "iam:TagRole",
        "iam:ListRolePolicies",
        "iam:ListInstanceProfilesForRole",
        "iam:AttachRolePolicy",
        "iam:DetachRolePolicy",
        "iam:ListAttachedRolePolicies"
      ],
      "Resource": "arn:aws:iam::YOUR_AWS_ACCOUNT_ID:role/lightfriend-*"
    },
    {
      "Sid": "IAMInstanceProfileManagement",
      "Effect": "Allow",
      "Action": [
        "iam:CreateInstanceProfile",
        "iam:DeleteInstanceProfile",
        "iam:GetInstanceProfile",
        "iam:TagInstanceProfile",
        "iam:AddRoleToInstanceProfile",
        "iam:RemoveRoleFromInstanceProfile"
      ],
      "Resource": "arn:aws:iam::YOUR_AWS_ACCOUNT_ID:instance-profile/lightfriend-*"
    },
    {
      "Sid": "IAMPassRoleToEC2",
      "Effect": "Allow",
      "Action": "iam:PassRole",
      "Resource": "arn:aws:iam::YOUR_AWS_ACCOUNT_ID:role/lightfriend-*",
      "Condition": {
        "StringEquals": {
          "iam:PassedToService": "ec2.amazonaws.com"
        }
      }
    }
  ]
}
```

4. Name the policy: `TerraformLightfriendPermissions`
5. Click "Create policy"

**Notes:**
- VPC operations are part of the EC2 service (use `ec2:CreateVpc`, not `vpc:CreateVpc`)
- This policy grants **minimum required permissions** for the Terraform configuration
- All IAM resources are scoped to `lightfriend-*` naming pattern for security
- The `iam:PassRole` condition restricts role passing to EC2 service only
- IAM permissions limited to: create/delete/read roles, attach managed policies, manage instance profiles

### 2.5 Verify Trust Policy

The AWS Console wizard should have automatically created the correct trust policy. Let's verify:

1. Go to the role you just created: IAM → Roles → `terraform-lightfriend-role`
2. Click the "Trust relationships" tab
3. Verify the trust policy looks like this:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Principal": {
        "Federated": "arn:aws:iam::YOUR_AWS_ACCOUNT_ID:oidc-provider/app.terraform.io"
      },
      "Action": "sts:AssumeRoleWithWebIdentity",
      "Condition": {
        "StringEquals": {
          "app.terraform.io:aud": "aws.workload.identity"
        },
        "StringLike": {
          "app.terraform.io:sub": "organization:YOUR_TF_CLOUD_ORG:project:*:workspace:*:run_phase:*"
        }
      }
    }
  ]
}
```

This restricts the role to only be assumable by workspaces in your Terraform Cloud organization. If it looks correct, you're all set!

### 2.6 Choose Region

We recommend `us-east-1` for Nitro Enclave support. Ensure your chosen region supports:
- Nitro Enclaves (c6a instances)
- Required instance types (c6a.2xlarge minimum)

Set `AWS_REGION` in your `.env` file.

### 2.7 Verify Nitro Enclave Availability

```bash
aws ec2 describe-instance-types \
  --instance-types c6a.2xlarge \
  --query "InstanceTypes[].EnclaveOptions.Supported"
```

Should return `[true]`.

## 3. Cloudflare Setup

### 3.1 Add Your Domain (if not already added)

If you don't already have a domain in Cloudflare:

1. Go to Cloudflare Dashboard
2. Click "Add a Site"
3. Enter your domain name
4. Choose the Free plan
5. Update your domain's nameservers at your registrar to point to Cloudflare
6. Wait for DNS propagation (can take up to 24 hours, usually much faster)

**Note**: The domain must be active in Cloudflare before proceeding.

### 3.2 Enable Zero Trust

1. Log in to Cloudflare Dashboard
2. Go to Zero Trust (left sidebar)
3. If prompted, set up your Zero Trust organization

### 3.3 Create API Token

1. Go to My Profile → API Tokens
2. Create Custom Token with these permissions:

| Permission | Access |
|------------|--------|
| Account / Cloudflare Tunnel | Edit |
| Zone / DNS | Edit |

3. For "Zone Resources", select your specific domain (or "All zones")
4. Save as `CLOUDFLARE_API_TOKEN`

**Notes:**
- Current Terraform configuration only creates tunnels and DNS records
- If you later add Zero Trust access policies (for admin endpoints, staging environments, etc.), you'll need to add:
  - `Account / Access: Apps and Policies | Edit`

### 3.4 Get Account and Zone IDs

1. Account ID: Found on the Overview page of your domain
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

# AWS Region
AWS_REGION=us-east-1

# Infrastructure - Cloudflare
CLOUDFLARE_API_TOKEN=<your-cloudflare-token>
CLOUDFLARE_ACCOUNT_ID=<your-account-id>
CLOUDFLARE_ZONE_ID=<your-zone-id>
CLOUDFLARE_DOMAIN=<your-domain.com>
```

**Note:** Terraform Cloud uses OIDC for AWS authentication and does not require AWS access keys in your `.env` file.

### 4.2 Initialize Terraform

```bash
cd terraform

# Login to Terraform Cloud
terraform login

# Initialize (will prompt for workspace if none exist with 'lightfriend' tag)
terraform init
```

**If prompted "No workspaces found":**
- This means no workspaces have the `lightfriend` tag in your organization
- Enter a workspace name when prompted: `lightfriend-dev-<yourname>`
- Terraform will create it with the correct tag automatically


### 4.3 Configure Workspace Variables

Before running terraform, configure your workspace variables as described in section 1.4:

1. Create the "Cloudflare Credentials" Variable Set (if not already done)
2. In your workspace, add the `environment` Terraform variable (e.g., `dev-eddie`)

### 4.4 Plan and Apply

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

Once the tunnel is running, you can access your environment at:

```bash
# API endpoint (environment-specific subdomain)
curl https://api-<environment>.<your-domain>/health

# Example for dev-eddie environment:
curl https://api-dev-eddie.example.com/health

# Frontend application
# https://app-<environment>.<your-domain>
```

The `<environment>` matches your workspace's `environment` variable (e.g., `dev-eddie`, `staging`, `prod`). This allows multiple environments to coexist on the same domain without conflicts.

**Note**: Terraform outputs will show your specific URLs after `terraform apply` completes.

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
├── main.tf              # Root module, backend config, required_providers
├── variables.tf         # Input variables
├── outputs.tf           # Output values
├── providers.tf         # Provider configuration
├── terraform.tfvars     # Variable values (git-ignored)
└── modules/
    ├── networking/      # VPC, subnets, security groups
    ├── compute/         # EC2 instance, Nitro config
    └── cloudflare/      # Tunnel, DNS (has own required_providers block)
```

**Note**: Each module that uses external providers must declare them in a `terraform.required_providers` block to avoid provider namespace resolution issues.

See each module's README for detailed configuration options.
