# DNS Migration Guide

## Current Setup (battle-cp.tech)

| URL | Serves | Host |
|-----|--------|------|
| `battle-cp.tech` | Frontend (Next.js) | Vercel |
| `www.battle-cp.tech` | Redirects to `battle-cp.tech` | Vercel |
| `battle-cp.vercel.app` | 301 redirects to `battle-cp.tech` | Vercel |
| `api.battle-cp.tech` | Backend API + WebSocket (Rust) | GCP VM (`34.121.245.90`) |
| `battle-cp.duckdns.org` | Full stack fallback | GCP VM (`34.121.245.90`) |

## DNS Records (manage.tech)

| Type | Host | Value | Purpose |
|------|------|-------|---------|
| A | `@` | `76.76.21.21` | Root → Vercel |
| CNAME | `www` | `cname.vercel-dns-0.com` | www → Vercel |
| A | `api` | `34.121.245.90` | API → GCP VM |

## When your .tech domain expires

If `battle-cp.tech` expires and you can't renew it immediately, switch back to DuckDNS:

### Step 1: Update backendUrls.ts

```typescript
// frontend/lib/backendUrls.ts
const PROD_API_BASE_URL = "https://battle-cp.duckdns.org";
```

### Step 2: Update all metadata URLs

Replace `battle-cp.tech` with `battle-cp.duckdns.org` in:
- `frontend/app/layout.tsx` (3 occurrences)
- `frontend/app/page.tsx` (3 occurrences)
- `frontend/app/robots.ts`
- `frontend/app/sitemap.ts`
- `frontend/app/game/[gameId]/page.tsx`
- `frontend/app/lobby/create/layout.tsx`
- `frontend/app/lobby/join/layout.tsx`

### Step 3: Update ALLOWED_ORIGINS

In these files, replace `battle-cp.tech` with `battle-cp.duckdns.org`:
- `.github/workflows/deploy-oracle.yml`
- `docker-compose.yml`
- `deploy_gcp.sh`

### Step 4: Update DNS

At your registrar (manage.tech), update the A record:
- Host: `@` → Value: `34.121.245.90` (GCP VM IP)

### Step 5: Verify

- Visit `battle-cp.duckdns.org` — should serve the full stack
- API calls go through `battle-cp.duckdns.org/api/` and `battle-cp.duckdns.org/ws/`

## DuckDNS Quick Reference

### How DuckDNS works
- Free dynamic DNS service
- You get a subdomain: `battle-cp.duckdns.org`
- IP updates via HTTP API: `https://www.duckdns.org/update?domains=battle-cp&token=YOUR_TOKEN&ip=YOUR_IP`
- No SSL by default — you need Let's Encrypt (certbot) on the VM

### DuckDNS token
Your DuckDNS token is set up on the GCP VM. To check/update it:
```bash
gcloud compute ssh battlecp-server --zone=us-central1-a --command="cat ~/duckdns/duck.sh"
```

### Updating DuckDNS IP (if VM IP changes)
```bash
# From your local machine:
curl "https://www.duckdns.org/update?domains=battle-cp&token=YOUR_TOKEN&ip=NEW_IP"
```

### Renewing SSL for DuckDNS
The certbot cert for `battle-cp.duckdns.org` auto-renews. To check:
```bash
gcloud compute ssh battlecp-server --zone=us-central1-a --command="sudo certbot certificates"
```

## GCP VM Details

- **Instance**: `battlecp-server` (zone: `us-central1-a`)
- **IP**: `34.121.245.90`
- **Project**: `battle-cp-prod`
- **SSH**: `gcloud compute ssh battlecp-server --zone=us-central1-a`
- **Services**: Docker (backend on :3000), Next.js frontend (on :3001), Nginx (80/443)
