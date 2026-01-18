output "tunnel_id" {
  description = "Cloudflare tunnel ID"
  value       = cloudflare_zero_trust_tunnel_cloudflared.main.id
}

output "tunnel_token" {
  description = "Cloudflare tunnel token for cloudflared service"
  value       = cloudflare_zero_trust_tunnel_cloudflared.main.tunnel_token
  sensitive   = true
}

output "tunnel_cname" {
  description = "Tunnel CNAME for DNS records"
  value       = "${cloudflare_zero_trust_tunnel_cloudflared.main.id}.cfargotunnel.com"
}

output "api_url" {
  description = "API endpoint URL"
  value       = "https://api-${var.environment}.${var.domain}"
}

output "app_url" {
  description = "App frontend URL"
  value       = "https://app-${var.environment}.${var.domain}"
}
