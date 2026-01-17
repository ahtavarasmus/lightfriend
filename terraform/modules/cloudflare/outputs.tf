output "tunnel_id" {
  description = "Cloudflare tunnel ID"
  value       = cloudflare_tunnel.main.id
}

output "tunnel_token" {
  description = "Cloudflare tunnel token for cloudflared service"
  value       = cloudflare_tunnel.main.tunnel_token
  sensitive   = true
}

output "tunnel_cname" {
  description = "Tunnel CNAME for DNS records"
  value       = "${cloudflare_tunnel.main.id}.cfargotunnel.com"
}
