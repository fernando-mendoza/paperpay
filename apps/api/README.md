# API de PaperPay

API Express en el puerto 4000. Carga los 52 artículos ficticios de `data/papers.json` (el frontend usa una copia en `apps/web/src/mocks/papers.json` para el modo `mock`).

## Inicio

Desde la raíz del repositorio:

```bash
cp apps/api/.env.example apps/api/.env
pnpm dev:api
pnpm test
```

La ruta `/api/health` indica que el proceso responde; no comprueba Horizon ni el facilitador.

## Despliegue

La API corre en una VPS con systemd y Caddy, detrás de Cloudflare: `https://paperpay-api.blockchains.tf`. Se despliega con `scripts/deploy-vps.sh` (corre `pnpm test`, compila, sube el release y verifica `/api/health`, con rollback si falla). Las variables de producción viven en `/etc/paperpay/api.env` en el servidor, no en el repositorio. Guía completa: [deploy/vps/README.md](../../deploy/vps/README.md). Antes corría en Railway; esa cuenta ya no se usa.

## Contrato HTTP

| Solicitud | Respuesta |
| --- | --- |
| `GET /api/health` | `200`: estado, red, modo, dirección configurada y `splitterContractId` (o `null`) |
| `GET /api/papers` | `200`: previews, sin `fullContentMarkdown` |
| `GET /api/papers/:id` sin credenciales | `402`: `{ preview }` y `payment-required` en Base64 JSON |
| `GET /api/papers/:id` con pago aceptado | `200`: `{ paper, accessToken, txHash }` y `payment-response` |
| `GET /api/papers/:id` con `Authorization: Bearer <jwt>` válido para ese artículo | `200`: `{ paper }` |
| `POST /api/papers/:id/verify` | Verifica un pago Pollar enviado previamente y devuelve `{ paper, accessToken, txHash }` |
| `POST /api/papers/:id/prepare` con `{ payer }` | Solo con el contrato de reparto activo. `200`: `{ transactionXdr, networkPassphrase, terms }` con la invocación `pay` ya simulada, lista para firmar. `404 SPLITTER_DISABLED` si no está activo |
| ID inexistente | `404`: `PAPER_NOT_FOUND` |

El requisito de pago declara `exact`, `stellar:testnet`, USDC SAC, `5000000` unidades de 7 decimales y la tesorería. `payment-signature` se decodifica como Base64 JSON (también acepta JSON directo). Una respuesta fallida devuelve `402 PAYMENT_FAILED`.

### Verificación Pollar por hash (v2)

Implementada y usada por el frontend en el flujo de Pollar.

Pollar firma y envía la transacción desde su propia wallet. Después, el frontend llama `POST /api/papers/:id/verify` con `{ txHash, signerPublicKey }` y la misma información en la cabecera `payment-signature` codificada como Base64 JSON:

```json
{
  "scheme": "exact",
  "network": "stellar:testnet",
  "txHash": "<hash Stellar de 64 caracteres>",
  "signerPublicKey": "G..."
}
```

La API consulta Horizon y exige que la transacción haya tenido éxito, sea reciente (300 segundos por defecto), pertenezca al firmante y contenga un pago exacto de 0.50 USDC Testnet a la tesorería configurada. Un mismo hash puede reintentarse para el mismo artículo y firmante, de modo que una respuesta de red perdida no obliga a pagar de nuevo; no puede desbloquear otro artículo.

La protección contra repetición se conserva solo en memoria porque la API aún no tiene una base de datos. Un reinicio o varias instancias pierden ese registro. Antes de producción debe sustituirse por un almacén persistente con una restricción única sobre el hash.

### Pago por el contrato de reparto 98/2

Se activa con `SPLITTER_CONTRACT_ID` (el contrato de [`contracts/splitter`](../../contracts/README.md)) y `SPLITTER_PUBLISHER_ADDRESS` (la cuenta que recibe la parte de la editorial; hoy, una editorial demo para todo el catálogo). La API no arranca si uno de los dos es inválido. No depende del modo de liquidación: el lector firma y paga la comisión de red.

1. El `402` agrega `extra.splitter = { contractId, function: "pay", publisher, prepareUrl }`.
2. El cliente pide `POST prepareUrl` con `{ payer }`. La API arma `pay(payer, paperId, publisher, 5000000)` y la simula por RPC (`STELLAR_RPC_URL`). Si la wallet no existe o no tiene USDC con trustline, responde `402`.
3. La wallet firma el XDR y el cliente lo manda como `signature` en `payment-signature`, igual que el pago clásico.
4. La API revisa lo firmado antes de enviarlo:
   - una sola operación: `pay` de ese contrato;
   - el artículo, la editorial y el monto que fija la API;
   - el pago sale de quien firma, y la firma es válida para esa wallet.
5. Lo envía por RPC y espera la confirmación. Entrega `{ paper, accessToken, txHash }` solo si el ledger trae el evento `paid` de ese artículo, con ese lector, editorial y monto.

Respuestas de error:

| Estado | Error | Cuándo |
| --- | --- | --- |
| `400` | `PAYMENT_FAILED` | El sobre no es el pago esperado. No se envía a la red. |
| `402` | `PAYMENT_FAILED` | La red lo rechazó. |
| `409` | `PAYMENT_ALREADY_USED` | Es un sobre ya confirmado hace más de 300 s. |
| `503` | `PAYMENT_FAILED` | La red pide reintentar. |
| `504` | `PAYMENT_PENDING` | Sigue sin confirmarse. La respuesta trae el `txHash`. |

**Respuesta perdida:** si el mismo sobre ya está confirmado y tiene menos de 300 s, la API no lo reenvía y vuelve a entregar el acceso. Es la misma ventana que la verificación por hash de Pollar: un sobre viejo es público en la red y no debe abrir el artículo a otros.

## Modos de liquidación

Por defecto la API intenta enviar la firma al URL configurado en `OPENZEPPELIN_CHANNELS_URL`. Exige una respuesta con `success: true` y un hash de 64 caracteres hexadecimales. **Ese contrato de integración no se ha probado con un pago real**; confirmar el formato y el endpoint del facilitador antes de usarlo.

`SELF_SETTLE=true` intenta enviar a Horizon un sobre de transacción clásica Stellar ya firmado. Comprueba que el origen coincida con `signerPublicKey` y que incluya un pago de al menos 0.50 USDC a la tesorería. No firma por el lector, no realiza una llamada Soroban y no asocia el pago criptográficamente con un artículo concreto.

El backend desplegado respondió `mode: SELF_SETTLE` el 25 de septiembre de 2026. La tesorería anunciada tenía 12 pagos confirmados de 0.50 USDC en Horizon; consulta [QA](../../docs/QA.md). Esto verifica transferencias reales en Testnet por la ruta compatible con el frontend Freighter, pero el ledger no demuestra por sí solo el desbloqueo de un artículo ni el reingreso con JWT. El lector paga la comisión de red en este modo.

`DEMO_PAYMENTS=true` acepta las firmas de prueba `unsigned-demo-signature`, `mock_*` y `demo_*`, y emite un hash `mock_tx_*` y JWT sin transferir fondos. Funciona solo con `NODE_ENV` distinto de `production`. Déjalo en `false` salvo para una demo local. Sin este modo, una firma de prueba recibe `402`.

En producción son obligatorios un `JWT_SECRET` aleatorio de al menos 32 caracteres (no uses valores de ejemplo: el repositorio es público), una `STELLAR_TREASURY_PUBLIC_KEY` válida y `OPENZEPPELIN_API_KEY` si se usa el facilitador. El ejemplo de `.env` usa una clave de tesorería ficticia. `generate:treasury` crea y fondea una cuenta de Testnet con Friendbot, pero no obtiene USDC ni verifica pagos.

## Pruebas

`pnpm test` ejecuta 80 pruebas locales. Incluyen la validación simulada de respuestas de Horizon para el flujo Pollar (monto exacto e intento de reutilizar un hash) y el pago por contrato. Para el contrato, la validación del sobre firmado, el envío y el cobro con un RPC simulado, el endpoint `/prepare` y la lectura de eventos `paid` usan eventos reales de testnet (`test/fixtures`). Ninguna de estas pruebas hace un pago externo.

`pnpm --filter @paperpay/api test:splitter` (con `SPLITTER_CONTRACT_ID`, `SPLITTER_PUBLISHER_ADDRESS` y `STELLAR_TREASURY_PUBLIC_KEY`) **sí paga en testnet**:
- crea un lector desechable con Friendbot y 1 USDC comprado en el DEX;
- recorre `402 → /prepare → firma → payment-signature` contra una API local;
- comprueba en Horizon que el lector pagó 0.50, la editorial recibió 0.49 y la tesorería 0.01;
- comprueba que el JWT reabre el artículo y que reenviar el mismo sobre no cobra dos veces. `pnpm --filter @paperpay/api test:testnet` verifica Friendbot, Horizon y el paywall de una API local, sin comprar artículos. `pnpm --filter @paperpay/api test:settle` usa una firma de prueba y solo puede desbloquear con `DEMO_PAYMENTS=true`.
