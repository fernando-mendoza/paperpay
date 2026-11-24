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

La unidad `paperpay-api.service` viaja con el release: si difiere de la instalada, el script la copia a `/etc/systemd/system/` y hace `daemon-reload` antes de reiniciar.

Se niega a correr si `apps/api`, `packages/shared` o el lockfile tienen cambios sin commitear. Corre `pnpm test` antes de subir nada. Tras reiniciar espera hasta 20 s a que `/api/health` responda; si no, restaura `/opt/paperpay.old` y sale con error.

## Rollback manual

```bash
sudo mv /opt/paperpay /opt/paperpay.bad && sudo mv /opt/paperpay.old /opt/paperpay
sudo systemctl restart paperpay-api
```

## Límites conocidos

- Los recibos viven en `/var/lib/paperpay/receipts.db` (SQLite; con `DynamicUser` el directorio real es `/var/lib/private/paperpay`). Sobreviven a reinicios, despliegues y rollbacks. **No hay respaldo fuera de la VPS:** si el disco se pierde, se pierden los recibos de Pollar; los del contrato se pueden reconstruir desde la red (evento `paid`).
- Una sola instancia. Para escalar hace falta un almacén compartido.
