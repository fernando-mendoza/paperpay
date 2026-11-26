<p align="center"><img src="apps/web/public/brand/paperpay-logo.svg" alt="PaperPay" height="56"></p>

# PaperPay

Paywall para artículos científicos: el lector paga un micropago en USDC sobre Stellar y desbloquea el artículo, sin crear cuenta ni usar tarjeta. Proyecto del track Stellar de Goya Hack 2026, construido con Next.js, Express y Stellar **Testnet**.

- **Sitio:** [paperpay.press](https://paperpay.press/) · réplica en Cloudflare Pages: [paperpay.blockchains.tf](https://paperpay.blockchains.tf/)
- **Versión:** 2.0.0 (v1: Freighter · v2: Pollar con login social)

El catálogo contiene **52 artículos ficticios de demostración** en 10 disciplinas; sus autores, DOI, resultados y cifras no son publicaciones reales. El precio de 0.50 USDC y el reparto 98% editorial / 2% PaperPay son propuestas iniciales.

## Estado actual

| Componente | Estado |
| --- | --- |
| Catálogo, vista previa y respuesta HTTP 402 | Implementado. El texto completo solo sale del servidor tras el pago. |
| Pago con **Freighter** (v1) | Implementado. Si la API anuncia el contrato de reparto (así está en [paperpay.blockchains.tf](https://paperpay.blockchains.tf/)), el lector firma la llamada `pay` del contrato. Si no, el navegador construye y firma una transacción clásica de 0.50 USDC y la API la envía a Horizon (`SELF_SETTLE`). |
| Pago con **Pollar** (v2) | Implementado. Login con Google o email; Pollar firma y envía el pago y la API lo verifica por hash en Horizon (`POST /api/papers/:id/verify`). Requiere la publishable key de Pollar. |
| Selector de wallet | Implementado. Con la key de Pollar configurada, el paywall ofrece Pollar y Freighter. |
| Acceso tras el pago | JWT de 24 horas por artículo, guardado en el navegador. El catálogo marca los artículos con acceso vigente. |
| Recibos persistentes | Implementado. Cada pago aceptado (Pollar, contrato o clásico) queda en SQLite ligado a su artículo, a la wallet que pagó y a quien recibe el acceso (distinto en un regalo), sobrevive a reinicios y se consulta en `GET /api/receipts/:txHash` con enlace a la transacción. En la API de la VPS desde el 24 de noviembre de 2026. |
| Pago con memo | `?memo=<texto>` en la URL de un artículo pone ese memo en la transacción (≤ 28 bytes). Como Soroban no admite memo, ese pago va por la transferencia clásica a la tesorería. Probado en testnet contra la API publicada. |
| Recomendar un artículo (referidos) | Implementado y activo en [paperpay.blockchains.tf](https://paperpay.blockchains.tf/) desde el 25 de noviembre de 2026. Con un enlace `?ref=<wallet>`, quien paga firma `pay_ref` (contrato v0.4.0) y esa wallet recibe la mitad de la comisión (0.005 USDC por lectura) en la misma transacción. Quien paga y la editorial ven lo mismo que en `pay`. Si la cuenta que recomienda no puede cobrar, la API arma `pay`. Primer pago con referido por el sitio publicado: [`d3a130f6…`](https://stellar.expert/explorer/testnet/tx/d3a130f6aff66928b9d4c54288901d0507d61b5995918753bfe83086754d1197) ([API](apps/api/README.md#lectura-recomendada-pay_ref)). |
| Regalar un artículo | Implementado y activo en [paperpay.blockchains.tf](https://paperpay.blockchains.tf/) desde el 24 de noviembre de 2026. Con Freighter, el lector paga por el contrato (`gift`) y comparte un enlace; quien lo abre lee el artículo sin wallet. Probado de punta a punta contra el sitio publicado. |
| Panel editorial (`/editorial`) | Lee de Horizon los pagos que llegan a la tesorería. En los pagos por contrato muestra el reparto 98/2 que se hizo en la red; en las transferencias directas (pago clásico y Pollar) el 98/2 es un cálculo visual. |
| Pagos simulados (`mock`) | Disponibles para demos locales; no mueven fondos. |
| Facilitador OpenZeppelin | Integración presente, sin liquidación real verificada. |
| Contrato Soroban de reparto | Activo en testnet en [paperpay.blockchains.tf](https://paperpay.blockchains.tf/) y su API desde el 23 de noviembre de 2026. [`contracts/splitter`](contracts/README.md) reparte 98/2 en la misma transacción y deja un comprobante con el id del artículo. Desde el 25 de noviembre el sitio usa la v0.4.0 (`CC5SIM…ADGA`), que suma `gift_batch`, `tip`, `refund`, `pay_authors` (coautores) y `pay_ref` (referidos), todas probadas en testnet. La v0.2.0 (`CAIMQY…HOR5`, del 23 al 25 de noviembre) quedó como contrato anterior: sus regalos y lecturas siguen valiendo. La API arma y simula `pay` (o `pay_ref`), revisa el sobre firmado y da acceso solo con el evento `paid` del artículo (`SPLITTER_CONTRACT_ID`). La web, con Freighter, pide `/prepare`, revisa el sobre antes de firmar y reenvía el mismo sobre si la confirmación tarda. El panel cuenta esas lecturas con su reparto real. Probado de punta a punta contra el sitio publicado con un lector desechable y, el 24 de noviembre, con un pago firmado en la extensión Freighter. Pollar y el sitio de Vercel siguen con transferencias completas a la tesorería. |

Las cabeceras `payment-required`, `payment-signature` y `payment-response` siguen el esquema de x402, pero el payload es propio y el proyecto no usa los paquetes oficiales de x402: no debe presentarse como interoperabilidad certificada con x402. El estado comprobado y sus límites están en [docs/QA.md](docs/QA.md).

## Despliegue

| Parte | Dónde | Cómo |
| --- | --- | --- |
| Frontend | Vercel ([paperpay.press](https://paperpay.press/)) | Workflow `deploy-web-vercel.yml` en cada push a `main` que toque `apps/web`, `packages/shared` o dependencias. Los PR generan preview. |
| Frontend (réplica) | Cloudflare Pages ([paperpay.blockchains.tf](https://paperpay.blockchains.tf/)) | `scripts/deploy-web-cf-pages.sh`: export estático de Next más una Pages Function que reenvía `/api/*` a la API. Detalle en [deploy/cloudflare-pages/README.md](deploy/cloudflare-pages/README.md). |
| API | VPS con systemd + Caddy detrás de Cloudflare ([paperpay-api.blockchains.tf](https://paperpay-api.blockchains.tf/api/health)) | `scripts/deploy-vps.sh`: prueba, compila, sube el release y verifica `/api/health` con rollback automático. Detalle en [deploy/vps/README.md](deploy/vps/README.md). |

El frontend usa el secreto del repositorio `VERCEL_TOKEN`; los IDs de proyecto que aparecen en los workflows no son secretos. La API estuvo en Railway hasta que esa cuenta dejó de usarse (noviembre de 2026): `deploy-api-railway.yml` quedó solo como disparo manual y ya no corre en cada push.

## Requisitos y arranque local

Node.js 22.13 o superior (la API usa `node:sqlite`) y pnpm 9 o superior (CI usa Node 22 y pnpm 10).

```bash
pnpm install
cp apps/web/.env.example apps/web/.env.local
pnpm dev:web
```

Abre `http://localhost:3000`. El `.env.example` del frontend apunta a la API desplegada con Freighter. Para una demo local sin backend ni wallet, usa `NEXT_PUBLIC_DATA_SOURCE=mock` y `NEXT_PUBLIC_WALLET=mock`: el catálogo y el desbloqueo funcionan en el navegador sin mover fondos.

Para levantar la API local:

```bash
cp apps/api/.env.example apps/api/.env
pnpm dev:api
curl http://localhost:4000/api/health
curl -i http://localhost:4000/api/papers/autonomous-ai-micropayments
```

`GET /api/papers/:id` responde `402` con la vista previa y la cabecera `payment-required`. Para conectar el frontend a esta API, usa `API_PROXY_TARGET=http://localhost:4000` en `apps/web/.env.local` y reinicia Next. Los detalles de cada modo están en los README de [la API](apps/api/README.md) y [el frontend](apps/web/README.md).

## Verificación

```bash
pnpm test                               # 123 pruebas de la API y 41 de la web
pnpm build:shared && pnpm build:api
pnpm --filter @paperpay/web typecheck
pnpm build
```

Las pruebas cubren el catálogo, el 402, JWT, el modo demo y la verificación por hash con respuestas de Horizon simuladas. No ejecutan un pago real. `pnpm --filter @paperpay/api test:testnet` comprueba Friendbot, Horizon y el 402 contra una API local, también sin pagar.

## Seguridad y configuración de producción

La API se niega a arrancar en `NODE_ENV=production` sin un `JWT_SECRET` de al menos 32 caracteres, una `STELLAR_TREASURY_PUBLIC_KEY` válida y, si no usa `SELF_SETTLE`, una `OPENZEPPELIN_API_KEY`. Genera el `JWT_SECRET` de forma aleatoria; los valores de los archivos de ejemplo y de la historia del repositorio son públicos y no deben usarse. La clave de tesorería de ejemplo no es una cuenta real. `DEMO_PAYMENTS` se desactiva siempre en producción.

Cada pago aceptado deja un recibo en SQLite (hash → artículo, wallet que paga y quien lee) que sobrevive a reinicios; es el anti-replay de Pollar, la base para reclamar regalos y se consulta en `GET /api/receipts/:txHash`. Es una sola instancia sin respaldo fuera de la VPS: antes de mainnet hace falta respaldo y, para escalar, un almacén compartido.

Un enlace de regalo es la llave del regalo: quien lo tiene abre el artículo, y la API no puede saber quién lo compartió. El hash y la cuenta que recibe son públicos en la red, pero sin la llave del enlace la API rechaza el reclamo.

## Documentación

- [API](apps/api/README.md): variables, endpoints y modos de liquidación.
- [Frontend](apps/web/README.md): modos de datos y de wallet, Pollar y panel editorial.
- [QA](docs/QA.md): qué está comprobado y qué falta.
- [Contratos](contracts/README.md): contrato Soroban de reparto 98/2.
- [Resumen ejecutivo](docs/RESUMEN_EJECUTIVO.md), [plan](docs/PLAN.md), [flujo de usuario](docs/FLUJO_USUARIO.md), [PRD](docs/PRD_BACKEND.md), [TDD](docs/TDD_BACKEND.md) y [panorama competitivo](<docs/PaperPay  panorama competitivo DeSci, Web3 y micropagos científicos.md>): documentos de planeación del 23 y 24 de septiembre. Describen la intención original; cada uno indica al inicio en qué difiere de lo implementado.

No hay licencia definida para este repositorio: el código es público, pero sin una licencia no se concede permiso de reutilización.
