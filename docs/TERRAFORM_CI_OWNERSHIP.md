# Terraform and CI Ownership

Production instance lifecycle is intentionally owned by GitHub Actions CI, not
Terraform.

## Source of Truth

Terraform owns slow-moving infrastructure:

- VPC, subnet, security groups
- IAM roles, policies, and instance profile
- S3 backup bucket and lifecycle rules
- EC2 launch template
- SSM parameters that describe deploy infrastructure:
  - `/lightfriend/launch-template-id`
  - `/lightfriend/subnet-id`
  - `/lightfriend/s3-bucket`
- Cloudflare tunnel and tunnel ingress config

The production apex DNS record for `lightfriend.ai` already exists in
Cloudflare outside Terraform. Terraform does not create it to avoid duplicate
record conflicts. The old `enclave.lightfriend.ai` Terraform-managed record was
removed during the migration.

CI owns live application instances:

- Launching a new EC2 instance from the Terraform-managed launch template
- Exporting the live enclave backup
- Restoring into the new enclave
- Verifying health and data count
- Updating `/lightfriend/instance-id`
- Switching traffic
- Terminating the old instance
- Rolling back and cleaning orphans on failure

## Why Terraform Does Not Own the Live EC2 Instance

The live enclave host cannot be safely replaced by Terraform alone. A valid
production cutover requires application-aware steps: export, restore, verify,
traffic switch, and rollback. CI already performs those steps and records the
current live instance in `/lightfriend/instance-id`.

If Terraform also owns an `aws_instance`, state eventually becomes stale because
CI replaces the host. A normal Terraform apply can then plan a confusing
create/destroy against a non-live instance, or overwrite the live SSM pointer.

## Applying Terraform

Normal Terraform applies are safe once the old `aws_instance.enclave` and
`aws_ssm_parameter.instance_id` have been detached from Terraform state.

Run this once before the first plan after removing Terraform's live EC2 resource:

```bash
terraform state rm \
  module.compute.aws_instance.enclave \
  module.compute.aws_ssm_parameter.instance_id
```

This only removes those objects from Terraform state. It does not terminate the
EC2 instance and it does not delete `/lightfriend/instance-id`.

Expected Terraform ownership after the migration:

- Terraform may update the launch template.
- Terraform may update S3 lifecycle, IAM, networking, and Cloudflare resources.
- Terraform must not create, destroy, or replace the live EC2 host.
- Terraform must not write `/lightfriend/instance-id`.

When `terraform apply` updates the launch template, the change affects the next
CI deploy. It does not restart or mutate the currently live instance by itself.

After the state detach, `terraform plan` should not mention destroying the old
EC2 instance or deleting `/lightfriend/instance-id`. If the plan says it will
destroy the instance or delete `/lightfriend/instance-id`, stop and do not apply.

## Deploying Application Changes

Push to the deploy branch and let `.github/workflows/docker.yml` perform the
blue-green deploy. CI reads the infrastructure parameters from SSM, launches a
new host from the latest launch template, verifies the restored enclave, then
updates `/lightfriend/instance-id`.

## Finding the Live Instance

Use SSM, not Terraform output:

```bash
aws ssm get-parameter \
  --name /lightfriend/instance-id \
  --query Parameter.Value \
  --output text
```

Terraform no longer outputs `instance_id` or `instance_public_ip` because those
values are deployment state, not infrastructure state.
