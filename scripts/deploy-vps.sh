#!/usr/bin/env bash
# Despliega la API en una VPS con systemd + Caddy (deploy/vps/README.md): prueba, compila,
# empaqueta con node_modules de producción, copia a /opt/paperpay (root, solo lectura),
# reinicia y comprueba /api/health. Si el health falla, vuelve al release anterior.
#   VPS_HOST=usuario@host bash scripts/deploy-vps.sh               # cambia la VPS en vivo
#   VPS_HOST=usuario@host bash scripts/deploy-vps.sh --no-restart  # solo copia (primera instalación)
# El release anterior queda en /opt/paperpay.old: rollback = intercambiar los dos y reiniciar.
set -euo pipefail
cd "$(dirname "$0")/.."
HOST="${VPS_HOST:?Define VPS_HOST=usuario@host}"
PORT="${PAPERPAY_PORT:-8110}"
[ -z "$(git status --porcelain -- apps/api packages/shared pnpm-lock.yaml)" ] ||
  { echo "apps/api, packages/shared o el lockfile tienen cambios sin commitear" >&2; exit 1; }
REV=$(git rev-parse --short HEAD)
OUT=$(mktemp -d)
trap 'rm -rf "$OUT"' EXIT

pnpm build:shared
pnpm test
pnpm build:api
pnpm --filter @paperpay/api deploy --prod --legacy "$OUT/api" >/dev/null
rm -rf "$OUT/api/node_modules/.pnpm/node_modules/@paperpay/api" # self-link a este checkout, roto en la VPS
echo "$REV" > "$OUT/REVISION"
cp deploy/vps/paperpay-api.service "$OUT/paperpay-api.service"

# bsdtar (macOS) agrega archivos AppleDouble y cabeceras xattr que GNU tar no entiende
MAC=(); tar --version | grep -q bsdtar && MAC=(--no-mac-metadata --no-xattrs)
COPYFILE_DISABLE=1 tar -C "$OUT" ${MAC[@]+"${MAC[@]}"} -czf - \
  REVISION paperpay-api.service api/dist api/data api/package.json api/node_modules |
  ssh "$HOST" 'set -e
    sudo rm -rf /opt/paperpay.new && sudo mkdir /opt/paperpay.new
    sudo tar -xzf - -C /opt/paperpay.new --no-same-owner
    sudo chown -R root:root /opt/paperpay.new && sudo chmod -R go-w /opt/paperpay.new
    sudo rm -rf /opt/paperpay.old
    if [ -d /opt/paperpay ]; then sudo mv /opt/paperpay /opt/paperpay.old; fi
    sudo mv /opt/paperpay.new /opt/paperpay
    echo "en la VPS: /opt/paperpay en $(cat /opt/paperpay/REVISION)"'

[ "${1:-}" = --no-restart ] && exit 0
ssh "$HOST" "set -e
  # La unidad viaja con el release; solo se reinstala si cambió (rollback no la toca: es compatible).
  if ! sudo cmp -s /opt/paperpay/paperpay-api.service /etc/systemd/system/paperpay-api.service; then
    sudo install -m 644 /opt/paperpay/paperpay-api.service /etc/systemd/system/paperpay-api.service
    sudo systemctl daemon-reload
    echo 'unidad systemd actualizada'
  fi
  sudo systemctl restart paperpay-api.service
  for i in \$(seq 1 20); do
    if curl -fsS 127.0.0.1:$PORT/api/health; then echo; exit 0; fi
    sleep 1
  done
  echo 'health falló: vuelvo al release anterior' >&2
  sudo journalctl -u paperpay-api.service -n 30 --no-pager >&2 || true
  if [ -d /opt/paperpay.old ]; then
    sudo mv /opt/paperpay /opt/paperpay.failed && sudo mv /opt/paperpay.old /opt/paperpay
    sudo rm -rf /opt/paperpay.failed && sudo systemctl restart paperpay-api.service
  fi
  exit 1"
