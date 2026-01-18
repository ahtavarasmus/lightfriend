# Changelog

All notable changes to Lightfriend will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Comprehensive Claude Code permissions configuration (`.claude/settings.local.json`)
- Development workflow skills for common tasks:
  - `lightfriend-db-migration` - Diesel migration workflow guide
  - `lightfriend-add-integration` - OAuth integration setup guide
  - `lightfriend-add-frontend-page` - Yew frontend page creation guide
- `/update-docs` command for documentation maintenance
- Organized documentation into `docs/` folder
- Complete Terraform infrastructure setup with AWS + Cloudflare
- Environment-specific subdomains for multi-environment support (e.g., `api-dev-eddie.example.com`)
- Comprehensive AWS IAM permissions policy for Terraform Cloud OIDC
- Terraform Cloud workspace configuration guide
- Domain variable support in compute module for dynamic hostname configuration

### Changed
- Streamlined `CLAUDE.md` from 373 to 97 lines (74% reduction)
- Moved detailed Docker setup to `docs/DOCKER_SETUP.md`
- Moved Matrix setup guide to `docs/MATRIX_SETUP_GUIDE.md`
- Updated all documentation references to new paths
- Migrated Cloudflare resources to non-deprecated providers (`cloudflare_zero_trust_tunnel_cloudflared`)
- Updated `INFRASTRUCTURE_SETUP.md` with complete step-by-step guide and troubleshooting
- Nitro Enclave allocator configuration: 4 vCPU and 8GB RAM (50% of c6a.2xlarge) for better workload distribution

### Fixed
- EC2 user_data script now resilient to Nitro Enclaves installation failures (continues with cloudflared setup)
- Cloudflared tunnel hostname pattern now matches Cloudflare DNS configuration (api-${environment}.${domain})
- Nitro Enclave CPU count constraint (must be multiple of 2 due to hyperthreading)
- Amazon Linux 2023 compatibility (removed deprecated `amazon-linux-extras` command)

## [2025-01-11] - SMS & Email Improvements

### Added
- Comprehensive end-to-end tests for SMS assistant
- Email template improvements with consistent Lightfriend branding
- Admin broadcast email functionality via Resend

### Changed
- Simplified SMS code structure for better maintainability
- Switched admin broadcast emails from Twilio to Resend
- Improved email templates with better HTML formatting

### Fixed
- Environment variable validation to use correct `MATRIX_SHARED_SECRET` variable name
- YouTube video route conflict by using query parameters instead of path segments

## [2025-01-10] - Docker & Infrastructure

### Added
- Complete Docker setup with multi-platform support (amd64 + arm64)
- Docker compose configuration for entire stack
- Auto-generation of bridge configuration files
- `just` command shortcuts for common operations
- Build optimizations: sccache, mold linker, cargo-chef
- Admin password change functionality
- Optional Resend email service integration
- `ADMIN_EMAILS` environment variable for admin notifications

### Changed
- Simplified environment variables (12 → 4 required for bridges)
- Improved build times with caching (15-20 min first build, 30-60s incremental)
- Core-only Docker compose option for backend-only development

### Fixed
- Server compatibility by removing mold linker from cargo config

## [2024-12] - Foundation

### Added
- Initial Rust backend (Axum web framework)
- Yew WebAssembly frontend
- Matrix homeserver integration (Synapse)
- mautrix bridge support:
  - WhatsApp
  - Signal
  - Messenger
  - Instagram
- Twilio SMS/Voice integration
- ElevenLabs voice AI integration
- Stripe payment processing
- Google OAuth (Calendar & Tasks)
- IMAP email monitoring
- SQLite + Diesel ORM
- JWT authentication
- Credit system with subscription tiers
- Background job scheduler
- AES-256-GCM encryption for sensitive data

### Security
- Encrypted storage for OAuth tokens, passwords, credentials
- HMAC webhook signature validation
- bcrypt password hashing
- Rate limiting per user

---

## Categories

Changes are grouped as follows:
- **Added** - New features
- **Changed** - Changes to existing functionality
- **Deprecated** - Soon-to-be removed features
- **Removed** - Removed features
- **Fixed** - Bug fixes
- **Security** - Security improvements
