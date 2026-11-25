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
| `POST /api/papers/:id/prepare` con `{ payer }` | Solo con el contrato de reparto activo. `200`: `{ transactionXdr, networkPassphrase, terms }` con la invocación `pay` ya simulada, lista para firmar. Con `{ payer, reader }` arma el regalo `gift`. `404 SPLITTER_DISABLED` si no está activo |
| `POST /api/papers/:id/gift` con `payment-signature` | Envía un regalo firmado (`gift`). `200`: `{ txHash, paperId, reader, settledAt }`. Quien paga no recibe el artículo ni un token |
| `POST /api/gifts/claim` con `{ txHash, reader, signedAt, signature }` | Quien abre el enlace de regalo firma con su llave. `200`: `{ paperId, paperTitle, accessToken, txHash, settledAt }`. `403` si la firma no es de la llave del regalo, `400` si tiene más de 5 min, `404` si no hay regalo con ese hash |
| `GET /api/receipts/:txHash` | `200`: el recibo guardado de ese pago, con `paperId`, título, wallet que pagó (`payer`), quien recibe el acceso (`reader`, distinto solo en un regalo), método (`pollar`, `splitter`, `classic`), monto en stroops, hora del ledger y enlace a stellar.expert. `404 RECEIPT_NOT_FOUND` si PaperPay no lo registró; `400` si el hash no es válido |
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

La API consulta Horizon y exige que la transacción haya tenido éxito, sea reciente (300 segundos por defecto), pertenezca al firmante y contenga un pago exacto de 0.50 USDC Testnet a la tesorería configurada. Un mismo hash puede reintentarse para el mismo artículo y firmante dentro de esa misma ventana de 300 segundos (contada desde la hora del ledger), de modo que una respuesta de red perdida no obliga a pagar de nuevo. Nunca puede desbloquear otro artículo. Pasada la ventana responde `409`: el hash y la wallet son públicos en la red, y sin ese límite cualquiera que los copie tendría acceso.

### Recibos persistentes

Cada pago aceptado deja un recibo en SQLite (`node:sqlite`, incluido en Node 22.13+; sin dependencias ni servidor aparte): hash de la transacción como clave primaria, artículo, wallet que paga, quien recibe el acceso, método, monto y hora del ledger. Un archivo de la versión 1 (sin `reader`) se migra solo al arrancar: sus recibos quedan a nombre de quien pagó. Ese paso corre en cada arranque, así que los recibos que escriba una versión anterior tras un rollback también se completan. Eso liga el pago a un artículo de forma permanente y sobrevive a reinicios y despliegues:

- **Pollar:** es el anti-replay. Un hash ya registrado para otro artículo o wallet responde `409` sin consultar Horizon.
- **Contrato y pago clásico:** se registran al liquidar. La prueba pública sigue en la red (en el contrato, el evento `paid` con el id del artículo).
- **Regalo:** se registra con `payer` y `reader` al confirmar; es lo que permite reclamarlo con el enlace (ver "Regalo por enlace").
- Si escribir el recibo falla después de cobrar, se registra el error y el lector recibe su artículo igual.

El archivo es `RECEIPTS_DB_PATH`; si no se define, `$STATE_DIRECTORY/receipts.db` (systemd lo fija con `StateDirectory=`) y, en local, `apps/api/.data/receipts.db`. En pruebas vive en memoria. En producción la API no arranca con `:memory:`. Node 24 imprime al arrancar `ExperimentalWarning: SQLite is an experimental feature`: es un aviso, no un error. Una sola instancia: varias necesitarían un almacén compartido.

### Pago por el contrato de reparto 98/2

Se activa con `SPLITTER_CONTRACT_ID` (el contrato de [`contracts/splitter`](../../contracts/README.md)) y `SPLITTER_PUBLISHER_ADDRESS` (la cuenta que recibe la parte de la editorial; hoy, una editorial demo para todo el catálogo). La API no arranca si uno de los dos es inválido. No depende del modo de liquidación: el lector firma y paga la comisión de red.

1. El `402` agrega `extra.splitter = { contractId, function: "pay", publisher, prepareUrl }`.
2. El cliente pide `POST prepareUrl` con `{ payer }`. La API arma `pay(payer, paperId, publisher, 5000000)` y la simula por RPC (`STELLAR_RPC_URL`). Si la wallet no existe o no tiene USDC con trustline, responde `402`. Además del XDR devuelve `claim`, un comprobante de ese sobre (HMAC del hash con una llave derivada de `JWT_SECRET`) que nunca llega a la red.
3. La wallet firma el XDR y el cliente lo manda como `signature` en `payment-signature`, con `claim` al lado.
4. La API revisa lo firmado antes de enviarlo:
   - una sola operación: `pay` de ese contrato;
   - el artículo, la editorial y el monto que fija la API;
   - el pago sale de quien firma, y la firma es válida para esa wallet.
5. Lo envía por RPC y espera la confirmación. Entrega `{ paper, accessToken, txHash }` solo si el ledger trae el evento `paid` de ese artículo, con ese lector, editorial y monto.

**Qué funciones del contrato usa la API.** Solo arma y acepta `pay` (lectura) y `gift` (regalo, sección siguiente); cualquier otra función del sobre firmado se rechaza con `400`. El contrato tiene más funciones (`pay_batch`, `gift_batch`, `tip`, `refund`, `pay_authors` y `pay_ref`; ver el [historial](../../contracts/CHANGELOG.md)), que funcionan en testnet pero que el sitio todavía no ofrece. Además, la API usa la v0.2.0 (`SPLITTER_CONTRACT_ID`), que solo tiene `pay`, `pay_batch` y `gift`.

Respuestas de error:

| Estado | Error | Cuándo |
| --- | --- | --- |
| `400` | `PAYMENT_FAILED` | El sobre no es el pago esperado. No se envía a la red. |
| `402` | `PAYMENT_FAILED` | La red lo rechazó. |
| `409` | `PAYMENT_ALREADY_USED` | Es un sobre ya confirmado hace más de 300 s sin su `claim`, o hace más de 24 h. |
| `503` | `PAYMENT_FAILED` | La red pide reintentar. |
| `504` | `PAYMENT_PENDING` | Sigue sin confirmarse. La respuesta trae el `txHash`. |

**Respuesta perdida:** si el mismo sobre ya está confirmado, la API no lo reenvía y vuelve a entregar el acceso, pero el sobre confirmado es público en la red y cualquiera puede copiarlo. Por eso:

- Sin `claim`, el acceso se entrega solo si el pago tiene menos de 300 s, la misma ventana que la verificación por hash de Pollar.
- Con el `claim` que `/prepare` le dio a quien pidió ese sobre, se entrega hasta 24 h después, lo que dura el acceso.

La comisión de inclusión lleva hasta 999 stroops al azar, así que dos pedidos iguales en el mismo segundo no dan el mismo sobre ni el mismo `claim`. Rotar `JWT_SECRET` invalida los comprobantes, igual que los accesos.

### Regalo por enlace (`gift`)

Uno paga y otro lee, sin que quien recibe tenga wallet. El `402` agrega `giftUrl` a `extra.splitter`; sin ese campo, la web no ofrece regalar.

1. El navegador de quien regala crea una llave nueva. Su dirección es el `reader` de `gift(payer, reader, paperId, publisher, 5000000)`; su secreto va en el enlace `/regalo#<txHash>.<secreto>`. El fragmento no viaja al servidor.
2. `POST prepareUrl` con `{ payer, reader }` arma y simula `gift`. La wallet lo firma y el cliente lo manda a `giftUrl` en `payment-signature`. La API revisa lo mismo que en `pay`, más que `reader` sea otra cuenta, y solo responde `200` si el evento `paid` trae a ese `reader`. Guarda el recibo con `payer` y `reader`.
3. Quien abre el enlace firma `giftClaimMessage(txHash, signedAt)` (`packages/shared`) con la llave y la manda a `/api/gifts/claim`. La API comprueba la firma (ed25519, vale 5 min), que el recibo sea un regalo (`reader ≠ payer`) y que `reader` sea esa llave, y entrega un JWT de 24 h. El enlace sirve otra vez cuando el acceso vence.

El hash y el `reader` son públicos en la red: sin la firma, cualquiera que mirara la red podría abrir el regalo. Quien tiene el enlace tiene el regalo. Si el recibo no está en SQLite (escritura fallida o disco perdido), el reclamo busca el regalo en el RPC, que retiene unos 7 días, y lo guarda. Reenviar a `giftUrl` un regalo ya confirmado solo devuelve su recibo.

## Modos de liquidación

Por defecto la API intenta enviar la firma al URL configurado en `OPENZEPPELIN_CHANNELS_URL`. Exige una respuesta con `success: true` y un hash de 64 caracteres hexadecimales. **Ese contrato de integración no se ha probado con un pago real**; confirmar el formato y el endpoint del facilitador antes de usarlo.

`SELF_SETTLE=true` intenta enviar a Horizon un sobre de transacción clásica Stellar ya firmado. Comprueba que el origen coincida con `signerPublicKey` y que incluya un pago de al menos 0.50 USDC a la tesorería. No firma por el lector, no realiza una llamada Soroban y no asocia el pago criptográficamente con un artículo concreto.

El backend desplegado respondió `mode: SELF_SETTLE` el 25 de septiembre de 2026. La tesorería anunciada tenía 12 pagos confirmados de 0.50 USDC en Horizon; consulta [QA](../../docs/QA.md). Esto verifica transferencias reales en Testnet por la ruta compatible con el frontend Freighter, pero el ledger no demuestra por sí solo el desbloqueo de un artículo ni el reingreso con JWT. El lector paga la comisión de red en este modo.

`DEMO_PAYMENTS=true` acepta las firmas de prueba `unsigned-demo-signature`, `mock_*` y `demo_*`, y emite un hash `mock_tx_*` y JWT sin transferir fondos. Funciona solo con `NODE_ENV` distinto de `production`. Déjalo en `false` salvo para una demo local. Sin este modo, una firma de prueba recibe `402`.

En producción son obligatorios un `JWT_SECRET` aleatorio de al menos 32 caracteres (no uses valores de ejemplo: el repositorio es público), una `STELLAR_TREASURY_PUBLIC_KEY` válida y `OPENZEPPELIN_API_KEY` si se usa el facilitador. El ejemplo de `.env` usa una clave de tesorería ficticia. `generate:treasury` crea y fondea una cuenta de Testnet con Friendbot, pero no obtiene USDC ni verifica pagos.

## Pruebas

`pnpm test` ejecuta 123 pruebas locales. Incluyen la validación simulada de respuestas de Horizon para el flujo Pollar (monto exacto, intento de reutilizar un hash, también después de un reinicio y fuera de la ventana), los recibos en SQLite (incluido cerrar y reabrir el archivo, migrar uno v1 y volver de un rollback), el pago por contrato y el regalo (sobre `gift`, reclamo con la firma de la llave, firma vencida o de otra llave, búsqueda en la red). Para el contrato, la validación del sobre firmado, el envío y el cobro con un RPC simulado, el endpoint `/prepare` y la lectura de eventos `paid` usan eventos reales de testnet (`test/fixtures`). Ninguna de estas pruebas hace un pago externo.

`pnpm --filter @paperpay/api test:splitter` (con `SPLITTER_CONTRACT_ID`, `SPLITTER_PUBLISHER_ADDRESS` y `STELLAR_TREASURY_PUBLIC_KEY`) **sí paga en testnet**:
- crea un lector desechable con Friendbot y 1 USDC comprado en el DEX;
- recorre `402 → /prepare → firma → payment-signature` contra una API local;
- comprueba en Horizon que el lector pagó 0.50, la editorial recibió 0.49 y la tesorería 0.01;
- comprueba que el JWT reabre el artículo y que reenviar el mismo sobre no cobra dos veces;
- con `LATE_RESEND_SECONDS=310`, espera y reenvía el sobre ya confirmado fuera de la ventana de 5 min: con `claim` da `200`, y sin él, `409`.

`pnpm --filter @paperpay/api test:testnet` verifica Friendbot, Horizon y el paywall de una API local, sin comprar artículos. `pnpm --filter @paperpay/api test:settle` usa una firma de prueba y solo puede desbloquear con `DEMO_PAYMENTS=true`.
