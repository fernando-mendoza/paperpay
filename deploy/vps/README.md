# API en una VPS (systemd + Caddy)

La API corría en Railway y esa cuenta ya no se usa. Ahora vive en una VPS Linux:

```
navegador ─► paperpay.press (Next en Vercel) ─ rewrite /api/* ─► https://paperpay-api.blockchains.tf
          ─► Cloudflare (proxied, oculta la IP) ─► VPS :443 ─► Caddy (TLS DNS-01)
          ─► 127.0.0.1:8110 paperpay-api (systemd, Node 24) ─► Horizon testnet
```

| Archivo | Qué es |
|---|---|
| `paperpay-api.service` | Unit de systemd: `DynamicUser`, sistema de archivos de solo lectura, sin capabilities, `MemoryMax=200M`, escucha en `127.0.0.1:8110`. |
| `Caddyfile.example` | Bloque del sitio para Caddy con TLS por DNS-01 de Cloudflare. |
| `api.env.example` | Variables de `/etc/paperpay/api.env`. |
| `../../scripts/deploy-vps.sh` | Prueba, compila, empaqueta y sube un release; health gate con rollback. |

## Requisitos de la VPS

- Node 24 en `/opt/node` (tarball oficial, sha256 verificado contra `SHASUMS256.txt`).
- Caddy 2 con el plugin `caddy-dns/cloudflare` y `CF_API_TOKEN` en su entorno.
- Firewall: 80/443 solo desde los rangos de Cloudflare; la API nunca se publica directo.
- Un usuario con `sudo` sin contraseña para el script de deploy.

## Primera instalación

1. Código: `VPS_HOST=usuario@host bash scripts/deploy-vps.sh --no-restart`.
2. Variables: copia `api.env.example` a `/etc/paperpay/api.env` (root, `0600`) y completa la tesorería. Genera el `JWT_SECRET` en el propio servidor, para que no pase por ningún otro lado:
   ```bash
   echo "JWT_SECRET=$(openssl rand -hex 32)" | sudo tee -a /etc/paperpay/api.env >/dev/null
   ```
   Los tokens duran 24 h. Rotarlo solo obliga a los lectores a volver a desbloquear.
3. Unit: copia `paperpay-api.service` a `/etc/systemd/system/`, `sudo systemd-analyze verify` y `sudo systemctl enable --now paperpay-api`. Comprueba `curl 127.0.0.1:8110/api/health`.
4. Público: registro DNS del subdominio hacia la VPS (proxied en Cloudflare), bloque de `Caddyfile.example` en el Caddyfile, `caddy validate` y `systemctl reload caddy`.

## Actualizar

```bash
VPS_HOST=usuario@host bash scripts/deploy-vps.sh
```

Se niega a correr si `apps/api`, `packages/shared` o el lockfile tienen cambios sin commitear. Corre `pnpm test` antes de subir nada. Tras reiniciar espera hasta 20 s a que `/api/health` responda; si no, restaura `/opt/paperpay.old` y sale con error.

## Rollback manual

```bash
sudo mv /opt/paperpay /opt/paperpay.bad && sudo mv /opt/paperpay.old /opt/paperpay
sudo systemctl restart paperpay-api
```

## Límites conocidos

- Los recibos anti-replay de los pagos por hash (Pollar) viven en memoria: un reinicio o un deploy los borra y un hash ya usado podría volver a aceptarse dentro de la ventana de `PAYMENT_VERIFICATION_MAX_AGE_SECONDS` (300 s). El almacén persistente es un pendiente antes de mainnet.
- Una sola instancia. Para escalar hace falta ese almacén compartido.
