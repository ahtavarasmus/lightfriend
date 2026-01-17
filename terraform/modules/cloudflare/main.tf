# Cloudflare module: Zero Trust tunnel and DNS
# Creates tunnel for secure access without exposing ports

resource "random_id" "tunnel_secret" {
  byte_length = 32
}

resource "cloudflare_tunnel" "main" {
  account_id = var.cloudflare_account_id
  name       = "${var.project_name}-${var.environment}"
  secret     = random_id.tunnel_secret.b64_std
}

resource "cloudflare_tunnel_config" "main" {
  account_id = var.cloudflare_account_id
  tunnel_id  = cloudflare_tunnel.main.id

  config {
    ingress_rule {
      hostname = "api.${var.domain}"
      service  = "http://localhost:3000"
    }
    ingress_rule {
      # Catch-all rule (required)
      service = "http_status:404"
    }
  }
}

resource "cloudflare_record" "api" {
  zone_id = var.cloudflare_zone_id
  name    = "api"
  value   = "${cloudflare_tunnel.main.id}.cfargotunnel.com"
  type    = "CNAME"
  proxied = true
  comment = "Lightfriend API - ${var.environment}"
}

# Optional: Frontend subdomain
resource "cloudflare_record" "app" {
  zone_id = var.cloudflare_zone_id
  name    = "app"
  value   = "${cloudflare_tunnel.main.id}.cfargotunnel.com"
  type    = "CNAME"
  proxied = true
  comment = "Lightfriend App - ${var.environment}"
}
