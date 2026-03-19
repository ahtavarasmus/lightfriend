# Final Steps: Local Verification to Running Nitro Enclave

All code is implemented. This guide covers everything from verifying the build locally (free) through to a running Nitro Enclave on AWS.

---

## Phase 1: Local Docker Verification (free, no AWS)

Test that the enclave image builds and all services start correctly before touching AWS.

### 1.1 Start Docker

Make sure Docker Desktop / Colima is running:
```bash
colima status    # or check Docker Desktop
colima start     # if not running
```

### 1.2 Build the image

```bash
just build-local
```

This runs the full multi-stage Dockerfile:
- Stage 1: Compile Rust backend
- Stage 2: Build Yew/WASM frontend
- Stage 3: Pull bridge binaries from upstream images
- Stage 4: Assemble runtime with PostgreSQL, Tuwunel, bridges, cloudflared, supervisord

Build takes 10-20 minutes on first run (cached after that). Watch for errors.

### 1.3 Create test .env

```bash
cd enclave
cp .env.example .env
```

Edit `enclave/.env` and fill in minimum required values:

```bash
# Generate random secrets for local testing
JWT_SECRET_KEY=$(openssl rand -base64 32)
JWT_REFRESH_KEY=$(openssl rand -base64 32)
ENCRYPTION_KEY=$(openssl rand -base64 32)
MATRIX_SHARED_SECRET=$(openssl rand -hex 32)
MATRIX_HOMESERVER_SHARED_SECRET=$(openssl rand -hex 32)
MATRIX_REGISTRATION_TOKEN=$(openssl rand -hex 16)
DOUBLE_PUPPET_SECRET=$(openssl rand -hex 32)
BACKUP_ENCRYPTION_KEY=$(openssl rand -base64 32)
ADMIN_EMAILS=your@email.com
ENVIRONMENT=development
```

Leave Stripe/Twilio/ElevenLabs empty - they're only required when `ENVIRONMENT=production`.

Do NOT set `CLOUDFLARE_TUNNEL_TOKEN` for local testing (cloudflared won't start, which is expected).

### 1.4 Start and verify

```bash
just up    # or: docker compose up
```

Watch the logs. You should see in order:

1. **PostgreSQL initializes** - creates 4 databases (lightfriend_db, whatsapp_db, signal_db, telegram_db)
2. **No VSOCK detected** - "No VSOCK device - running in direct network mode" (expected locally)
3. **Config templates generated** - tuwunel.toml, bridge configs
4. **Bridge registrations generated** - whatsapp, signal, telegram registration YAMLs
5. **Supervisord starts** - PostgreSQL, then Tuwunel, then bridges, then backend
6. **Tuwunel ready** - responds on port 8008
7. **Backend starts** - runs migrations, listens on port 3000
8. **No cloudflared** - does not start (no tunnel token, expected)

Test endpoints in another terminal:
```bash
# Matrix homeserver
curl http://localhost:8008/_matrix/client/versions
# Expected: JSON with supported Matrix versions

# Backend (may take 30-60s on first start for migrations)
curl http://localhost:3000/
# Expected: serves frontend HTML or redirect
```

### 1.5 Clean up

```bash
just down    # or: docker compose down -v
```

The `-v` flag removes volumes so next start is fresh.

**If this phase passes, the enclave image is good.** Move to Phase 2.

---

## Phase 2: Terraform Setup and Dry Run (free)

### 2.1 Terraform Cloud setup

If not already done (see `docs/INFRASTRUCTURE_SETUP.md` for full details):

1. Create account at [app.terraform.io](https://app.terraform.io)
2. Create organization (e.g. `lightfriend-ai`)
3. Create workspace `lightfriend-prod` (CLI-driven workflow)
4. Add tag `lightfriend` to the workspace
5. Run `terraform login` locally

### 2.2 AWS IAM setup

If not already done:

1. Create OIDC Identity Provider in AWS IAM:
   - Provider URL: `https://app.terraform.io`
   - Audience: `aws.workload.identity`

2. Create IAM role `terraform-lightfriend-role` with the full permissions policy from `docs/INFRASTRUCTURE_SETUP.md` section 2.4

   **Important**: The policy must include these statements (added for S3 backups and GitHub OIDC):
   ```json
   {
     "Sid": "S3BackupBucket",
     "Effect": "Allow",
     "Action": [
       "s3:CreateBucket", "s3:DeleteBucket",
       "s3:GetBucketPolicy", "s3:PutBucketPolicy",
       "s3:GetEncryptionConfiguration", "s3:PutEncryptionConfiguration",
       "s3:GetBucketTagging", "s3:PutBucketTagging",
       "s3:ListBucket", "s3:GetBucketAcl", "s3:GetBucketVersioning"
     ],
     "Resource": "arn:aws:s3:::lightfriend-*"
   },
   {
     "Sid": "IAMPolicyAndOIDC",
     "Effect": "Allow",
     "Action": [
       "iam:PutRolePolicy", "iam:GetRolePolicy", "iam:DeleteRolePolicy",
       "iam:CreateOpenIDConnectProvider", "iam:DeleteOpenIDConnectProvider",
       "iam:GetOpenIDConnectProvider", "iam:TagOpenIDConnectProvider"
     ],
     "Resource": [
       "arn:aws:iam::YOUR_ACCOUNT_ID:role/lightfriend-*",
       "arn:aws:iam::YOUR_ACCOUNT_ID:oidc-provider/token.actions.githubusercontent.com"
     ]
   }
   ```

3. Configure Terraform Cloud workspace variables:

   **Variable Set "AWS Dynamic Credentials"** (environment variables):
   - `TFC_AWS_PROVIDER_AUTH` = `true`
   - `TFC_AWS_RUN_ROLE_ARN` = `arn:aws:iam::YOUR_ACCOUNT_ID:role/terraform-lightfriend-role`

### 2.3 Cloudflare setup

If not already done:

1. Domain active in Cloudflare with Zero Trust enabled
2. Create API token: My Profile -> API Tokens -> Custom Token
   - Permissions: `Account / Cloudflare Tunnel: Edit` + `Zone / DNS: Edit`
3. Note your Account ID and Zone ID (domain Overview page)

4. Configure Terraform Cloud workspace variables:

   **Variable Set "Cloudflare Credentials"**:
   - `CLOUDFLARE_API_TOKEN` (environment variable, sensitive)
   - `cloudflare_account_id` (terraform variable, sensitive)
   - `cloudflare_zone_id` (terraform variable, sensitive)
   - `cloudflare_domain` (terraform variable) - e.g. `lightfriend.ai`

   **Per-workspace variables**:
   - `environment` (terraform variable) = `prod`

### 2.4 Terraform plan (dry run)

```bash
cd terraform
terraform init
terraform plan
```

This shows exactly what would be created, with no cost. Verify:

- [ ] 1 VPC + 1 public subnet + internet gateway + route table
- [ ] 1 security group (outbound only, no inbound ports)
- [ ] 1 EC2 instance: c6a.2xlarge, Nitro Enclave enabled, 50GB gp3 encrypted
- [ ] 1 S3 bucket: `lightfriend-prod-backups` with AES256 encryption
- [ ] 1 Cloudflare tunnel + 1 CNAME DNS record
- [ ] IAM roles: enclave (SSM + S3), GitHub deploy (SSM + S3)
- [ ] GitHub OIDC provider for Actions
- [ ] No unexpected resources

**If plan looks correct, move to Phase 3.**

---

## Phase 3: Deploy Infrastructure (costs start here)

### 3.1 Apply terraform

```bash
terraform apply
```

Review the plan one more time, then type `yes`.

Takes 2-5 minutes. When done, save the outputs:

```bash
# Instance ID (for SSM and GitHub secrets)
terraform output instance_id

# Cloudflare tunnel token (for .env)
terraform output -raw tunnel_token

# S3 bucket name (for .env)
terraform output backup_bucket_name

# GitHub deploy role ARN (for GitHub secrets)
terraform output github_deploy_role_arn

# Your app URL
terraform output url
```

**Write these down - you'll need them in the next steps.**

### 3.2 Set up AWS Budget alarm (recommended)

Go to AWS Console -> Billing -> Budgets -> Create Budget:
- Monthly budget: set your limit (e.g. $200)
- Alert at 80% with email notification
- Optionally add a Budget Action at 100% to stop EC2 instances

### 3.3 Wait for EC2 user_data to finish

The EC2 instance runs `user_data.sh` on first boot, which installs Docker, Nitro CLI, tinyproxy, and all VSOCK services. This takes 3-5 minutes.

Check progress:
```bash
aws ssm start-session --target <instance_id>

# Once connected:
sudo tail -f /var/log/cloud-init-output.log
# Wait until you see "Lightfriend enclave host setup complete!"

# Verify VSOCK services are running:
sudo systemctl status vsock-proxy-bridge
sudo systemctl status vsock-config-server
sudo systemctl status vsock-backup-receiver
sudo systemctl status vsock-seed-server

# Verify Nitro CLI works:
nitro-cli describe-enclaves
# Expected: empty list []
```

---

## Phase 4: Create .env and Launch Enclave

### 4.1 Create .env on the EC2 host

While SSM'd into the instance:

```bash
sudo nano /opt/lightfriend/.env
```

Paste your .env contents. At minimum you need:

```bash
# ── Core (always required) ──────────────────────────────────────────────
JWT_SECRET_KEY=<generate: openssl rand -base64 32>
JWT_REFRESH_KEY=<generate: openssl rand -base64 32>
ENCRYPTION_KEY=<generate: openssl rand -base64 32>

# ── Matrix ──────────────────────────────────────────────────────────────
MATRIX_SHARED_SECRET=<generate: openssl rand -hex 32>
MATRIX_HOMESERVER_SHARED_SECRET=<same as MATRIX_SHARED_SECRET>
MATRIX_REGISTRATION_TOKEN=<generate: openssl rand -hex 16>
DOUBLE_PUPPET_SECRET=<generate: openssl rand -hex 32>

# ── Admin ───────────────────────────────────────────────────────────────
ADMIN_EMAILS=your@email.com
BOOTSTRAP_ADMIN_PASSWORD=<choose a password>
BOOTSTRAP_ADMIN_PHONE=<your phone number>

# ── Runtime ─────────────────────────────────────────────────────────────
ENVIRONMENT=development
# Set to "production" only when Stripe/Twilio/ElevenLabs keys are ready
FRONTEND_URL=https://<subdomain>.<domain>
SERVER_URL=https://<subdomain>.<domain>

# ── Enclave ─────────────────────────────────────────────────────────────
BACKUP_ENCRYPTION_KEY=<generate: openssl rand -base64 32>
CLOUDFLARE_TUNNEL_TOKEN=<from terraform output -raw tunnel_token>
S3_BACKUP_BUCKET=<from terraform output backup_bucket_name>

# ── Production-required (add when ready) ────────────────────────────────
# STRIPE_SECRET_KEY=
# STRIPE_PUBLISHABLE_KEY=
# STRIPE_WEBHOOK_SECRET=
# ... (see enclave/.env.example for full list)
```

**Save all generated secrets somewhere safe (password manager).** If you lose ENCRYPTION_KEY, user data is unrecoverable.

### 4.2 Launch the enclave

```bash
sudo /opt/lightfriend/launch-enclave.sh
```

This will:
1. Pull the Docker image from Docker Hub
2. Build the EIF (Enclave Image File) from the Docker image - takes 5-10 minutes
3. Terminate any existing enclave
4. Launch the new enclave with 8GB RAM, 4 vCPUs, CID 16

Output should show PCR values and enclave state "RUNNING".

### 4.3 Monitor startup

```bash
# View enclave console output (all stdout/stderr from inside)
nitro-cli console --enclave-id $(nitro-cli describe-enclaves | jq -r '.[0].EnclaveID')
```

You should see the entrypoint.sh output:
1. Environment loaded from host via VSOCK
2. No backup available (first boot)
3. PostgreSQL initializes
4. Databases created
5. Configs generated, bridges registered
6. Supervisord starts all services
7. Cloudflare tunnel started

### 4.4 Verify

```bash
# From your local machine - test via Cloudflare tunnel
curl https://<subdomain>.<domain>/_matrix/client/versions
# Expected: JSON with Matrix versions

curl https://<subdomain>.<domain>/
# Expected: Frontend HTML
```

If these work, your enclave is live.

---

## Phase 5: GitHub CI/CD Setup

### 5.1 Create production environment

Go to your GitHub repo -> Settings -> Environments -> New environment:
- Name: `production`
- Check "Required reviewers" -> add yourself
- Save

This means pushes to master build the image automatically, but deploy pauses for your manual approval.

### 5.2 Set GitHub secrets

```bash
# Docker Hub (for pushing enclave image)
gh secret set DOCKERHUB_USERNAME
gh secret set DOCKERHUB_TOKEN

# AWS (for SSM deploy)
gh secret set AWS_DEPLOY_ROLE_ARN --body "<github_deploy_role_arn output>"
gh secret set AWS_INSTANCE_ID --body "<instance_id output>"
gh secret set AWS_REGION --body "us-east-1"
```

### 5.3 Test the pipeline

Push a change to master. The workflow should:
1. Build and push Docker image to Docker Hub
2. Deploy job pauses, waiting for your approval in Actions UI
3. After approval: runs `launch-enclave.sh` on EC2 via SSM

---

## Phase 6: First Backup Test

Once the enclave is running with data:

### 6.1 Trigger export

```bash
# SSM into the EC2 host
aws ssm start-session --target <instance_id>

# The export runs inside the enclave. We trigger it by connecting to the
# enclave's console and running the script. Since we can't exec into a
# Nitro Enclave directly, export.sh runs as part of a supervisorctl command
# or is triggered via an API endpoint.

# For now, the backup is created on enclave shutdown/restart cycle.
# To test manually, you'd need to add an API endpoint or trigger mechanism.
```

### 6.2 Verify backup appears on host

```bash
ls -la /opt/lightfriend/backups/
# Should show a .tar.gz.enc file
```

### 6.3 Upload to S3

```bash
sudo /opt/lightfriend/upload-backup.sh
# Should print: Uploaded: s3://lightfriend-prod-backups/backups/backup-...
```

### 6.4 Test restore on a fresh enclave

```bash
# Download backup to seed directory
sudo /opt/lightfriend/download-backup.sh

# Relaunch enclave - it will fetch the backup via VSOCK and restore
sudo /opt/lightfriend/launch-enclave.sh

# Watch console for restore output
nitro-cli console --enclave-id $(nitro-cli describe-enclaves | jq -r '.[0].EnclaveID')
# Should see: "Full encrypted backup detected", restore steps, "Full restore complete"
```

---

## Troubleshooting

### Enclave won't start
```bash
# Check Nitro allocator
sudo systemctl status nitro-enclaves-allocator
cat /etc/nitro_enclaves/allocator.yaml
# Must have: memory_mib: 8192, cpu_count: 4

# Check Docker
sudo systemctl status docker
docker images | grep lightfriend
```

### VSOCK services not responding
```bash
sudo systemctl status vsock-config-server
sudo journalctl -u vsock-config-server -n 20

# Restart all VSOCK services
for svc in vsock-proxy-bridge vsock-config-server vsock-backup-receiver vsock-seed-server; do
    sudo systemctl restart $svc
done
```

### Cloudflare tunnel not connecting
```bash
# Check enclave console for cloudflared errors
nitro-cli console --enclave-id $(nitro-cli describe-enclaves | jq -r '.[0].EnclaveID') | grep -i cloudflare

# Verify tunnel token is set in .env
grep CLOUDFLARE_TUNNEL_TOKEN /opt/lightfriend/.env

# Verify outbound proxy works (tinyproxy)
sudo systemctl status tinyproxy
curl -x http://127.0.0.1:3128 https://api.cloudflare.com/cdn-cgi/trace
```

### Backend won't start inside enclave
```bash
# Check if PostgreSQL is running (in enclave console output)
# Look for "PostgreSQL is ready" in the logs
# If migrations fail, check the backend log for errors

# Common issue: ENCRYPTION_KEY not set or invalid
grep ENCRYPTION_KEY /opt/lightfriend/.env
```

### Terraform plan fails
```bash
# Check AWS credentials
terraform plan 2>&1 | head -20
# Look for "Error: configuring Terraform AWS Provider" -> check OIDC setup

# Check Cloudflare credentials
# Look for "Error: failed to create API client" -> check CLOUDFLARE_API_TOKEN
```

---

## Cost Summary

| Resource | Monthly Cost (us-east-1) |
|----------|--------------------------|
| EC2 c6a.2xlarge (on-demand) | ~$185 |
| EBS 50GB gp3 | ~$4 |
| S3 (backups, minimal) | <$1 |
| Cloudflare tunnel | Free |
| Data transfer (estimate) | ~$5-10 |
| **Total** | **~$195-200/mo** |

Consider Reserved Instances or Savings Plans after confirming the setup works.
