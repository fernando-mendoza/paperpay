#!/usr/bin/env bash
# Publica el frontend como sitio estático en Cloudflare Pages (deploy/cloudflare-pages/README.md):
# typecheck, export estático de Next, _headers y la Pages Function que reenvía /api/* a la API.
#   CLOUDFLARE_ACCOUNT_ID=<id> CLOUDFLARE_API_TOKEN=<token> bash scripts/deploy-web-cf-pages.sh
# Opcionales: CF_PAGES_PROJECT (paperpay) · CF_PAGES_BRANCH (main = producción; otra = preview)
#   NEXT_PUBLIC_SITE_URL · NEXT_PUBLIC_WALLET (freighter) · NEXT_PUBLIC_POLLAR_PUBLISHABLE_KEY
set -euo pipefail
cd "$(dirname "$0")/.."
# Sin la cuenta fija, wrangler puede elegir otra que el token también vea y fallar con un 10000.
: "${CLOUDFLARE_ACCOUNT_ID:?Define CLOUDFLARE_ACCOUNT_ID}"
: "${CLOUDFLARE_API_TOKEN:?Define CLOUDFLARE_API_TOKEN}"
PROJECT="${CF_PAGES_PROJECT:-paperpay}"
BRANCH="${CF_PAGES_BRANCH:-main}"
WRANGLER="wrangler@${WRANGLER_VERSION:-4.158.0}"
[ -z "$(git status --porcelain -- apps/web packages/shared deploy/cloudflare-pages pnpm-lock.yaml)" ] ||
  { echo "apps/web, packages/shared, deploy/cloudflare-pages o el lockfile tienen cambios sin commitear" >&2; exit 1; }
REV=$(git rev-parse --short HEAD)

pnpm build:shared
pnpm --filter @paperpay/web typecheck
rm -rf apps/web/out
# Todo NEXT_PUBLIC_* se hornea al compilar. NEXT_PUBLIC_API_URL vacío = /api/* relativo, que
# atiende la Pages Function.
STATIC_EXPORT=1 \
  NEXT_PUBLIC_DATA_SOURCE=api \
  NEXT_PUBLIC_API_URL= \
  NEXT_PUBLIC_WALLET="${NEXT_PUBLIC_WALLET:-freighter}" \
  NEXT_PUBLIC_SITE_URL="${NEXT_PUBLIC_SITE_URL:-https://paperpay.blockchains.tf}" \
  NEXT_PUBLIC_POLLAR_PUBLISHABLE_KEY="${NEXT_PUBLIC_POLLAR_PUBLISHABLE_KEY:-}" \
  pnpm --filter @paperpay/web build
cp deploy/cloudflare-pages/_headers apps/web/out/_headers

# wrangler toma functions/ del directorio desde donde corre.
cd deploy/cloudflare-pages
pnpm dlx "$WRANGLER" pages deploy ../../apps/web/out \
  --project-name "$PROJECT" --branch "$BRANCH" \
  --commit-hash "$REV" --commit-message "$(git log -1 --format=%s)" --commit-dirty=false
