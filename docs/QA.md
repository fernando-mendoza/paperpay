# Estado de QA

## Revisión del 24 de noviembre de 2026: recibos persistentes (sin desplegar)

Cada pago aceptado deja un recibo en SQLite (`node:sqlite`): hash → artículo, wallet, método, monto y hora del ledger.

**Comprobado:**

- `pnpm test`: 98 pruebas de la API y 28 de la web. Las nuevas cubren el recibo de Pollar con la hora de Horizon, el mismo hash para otro artículo después de un reinicio (409 sin consultar Horizon), el reintento fuera de la ventana de 5 minutos (409), un hash del contrato usado en `/verify` (409), cerrar y reabrir el archivo, el endpoint `GET /api/receipts/:txHash` y que un fallo al escribir no le quite el artículo al lector.
- Mutaciones: quitar la ventana, pisar recibos, no consultar el almacén o aceptar un hash del contrato en `/verify` rompen 1, 1, 5 y 1 pruebas.
- Testnet con una API local escribiendo a un archivo: pago por contrato [`00e4300b…`](https://stellar.expert/explorer/testnet/tx/00e4300bbd639d2886c67147cb344aeacd4a77b2329f031ac5de7e8208ecea4c) (0.49 / 0.01). Otra API arrancada después sobre el mismo archivo devolvió el recibo con su artículo, wallet, método `splitter` y hora del ledger.

**No comprobado:** un pago real de Pollar con el recibo en disco, y el comportamiento en la VPS con `StateDirectory=`.

## Revisión del 23 de noviembre de 2026: contrato de reparto en producción

Desde esta fecha la API de la VPS (`paperpay-api.blockchains.tf`, fork `6f63857`) tiene el contrato de reparto `CAIMQY…HOR5` (v0.2.0, testnet), y [paperpay.blockchains.tf](https://paperpay.blockchains.tf/) paga con él cuando el lector usa Freighter.

**Comprobado:**

- `pnpm test`: 80 pruebas de la API y 28 de la web.
- `/api/health` publicado anuncia `splitterContractId`. El `402` trae `extra.splitter`, y `POST /api/papers/:id/prepare` llega a la API a través de la Pages Function.
- `test:e2e` de la web contra el sitio publicado, con un lector desechable de Friendbot y Freighter simulado con su llave: 402 → `/prepare` → firma → artículo en 5.0 s, y el JWT reabre el artículo. En la tx [`96dd614e…`](https://stellar.expert/explorer/testnet/tx/96dd614e5a3b13c2e0d7dfc58d1e5b6457f554dc22754e25ce89a719293e74d9), Horizon muestra 0.49 USDC a la editorial demo y 0.01 a la tesorería. El panel la cuenta como lectura por contrato.

- 24 de noviembre: pago firmado en la extensión Freighter (LibreWolf, Testnet) en el sitio publicado. Tx [`3d7b2c56…`](https://stellar.expert/explorer/testnet/tx/3d7b2c5625e5869a174925aa8ca1d221c5edb9e54843f6638eaf25060b9c88f4): una llamada al contrato, 0.49 USDC a la editorial demo y 0.01 a la tesorería; comisión de red 0.0012421 XLM. El sitio mostró el recibo y el artículo completo.

**Límites conocidos:** Pollar y el sitio de Vercel siguen con transferencias completas a la tesorería. Hasta el 24 de noviembre, si la confirmación tardaba y el lector reenviaba el mismo sobre después de 5 minutos, la API ya no entregaba el acceso; ahora lo entrega con el comprobante de `/prepare` (ver `apps/api/README.md`, "Respuesta perdida"). Ese arreglo está en producción desde el 24 de noviembre (API `fefe306` en la VPS y el sitio de Pages): `/prepare` publicado devuelve `claim` y `test:e2e` contra el sitio pagó de nuevo ([`b8806395…`](https://stellar.expert/explorer/testnet/tx/b88063951a901e88a3daaf75d680cf24b121fd5eb786cfb3559d35a692408cc3), 4.5 s). El reenvío tardío con comprobante se probó en testnet contra una API local, no contra la publicada.

## Revisión del 27 de septiembre de 2026 (v2.0.0)

Desde la revisión del 25 de septiembre se agregó el pago con Pollar (v2) y el despliegue automático de web y API desde `main`.

**Comprobado en esta revisión:**

- `pnpm test`: 37 pruebas de la API pasan. Incluyen la verificación por hash de Pollar con respuestas de Horizon simuladas: pago válido con reintento idempotente, monto distinto de 0.50 USDC, intento de reutilizar un hash en otro artículo y fallas o timeouts de Horizon.
- `pnpm --filter @paperpay/web typecheck` pasa.
- El workflow de despliegue del frontend terminó con éxito para el commit `8cc686f` (27 sep).

**No comprobado en esta revisión:** no se ejecutó una compra nueva con Freighter ni con Pollar, y no se consultaron el sitio ni la API en producción, porque el entorno de revisión no tenía acceso a esos dominios. El historial del repositorio declara pruebas manuales del flujo de Pollar en Testnet (commit `6970129`) que aquí no se repitieron.

**Límites conocidos de v2:**

- La protección contra reutilizar un hash de Pollar se guarda en memoria: un reinicio de la API o varias instancias la pierden.
- El pago de Pollar se verifica contra la tesorería y el monto, pero el ledger no registra a qué artículo corresponde.

## Revisión del 25 de septiembre de 2026

El repositorio cambió después de la revisión inicial del 24 de septiembre: ahora incluye firma de transacciones con Freighter y una ruta de liquidación clásica en Stellar Testnet.

### Comprobado

- API: 28 pruebas locales de catálogo, 402, JWT, parsing y control del modo demo. El caso de desbloqueo simula la respuesta de liquidación.
- Compilación del monorepo con webpack y arranque local de la API en `NODE_ENV=production` durante la revisión anterior.
- El backend desplegado respondió `mode: SELF_SETTLE` y anunció la tesorería `GAB2NXBPZMJEPZPXMAAEZDJA7SB6ZQRHTT2KF4Z7LMOEAJKSP4JBCWRV` en [su health check](https://paperpay-backend-production.up.railway.app/api/health).
- [Horizon Testnet](https://horizon-testnet.stellar.org/accounts/GAB2NXBPZMJEPZPXMAAEZDJA7SB6ZQRHTT2KF4Z7LMOEAJKSP4JBCWRV/payments?order=desc&limit=20) registraba 12 pagos entrantes de 0.50 USDC cada uno (6 USDC en total) desde tres cuentas entre `2026-09-25T02:27:22Z` y `2026-09-25T05:35:52Z`. Se filtró el emisor USDC `GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5`.
- La [transacción más reciente consultada](https://horizon-testnet.stellar.org/transactions/37618096068d6f9f4b5785e9681a917b85ad1b0483c9a15eddafc1b9c5bdf80a) tiene `successful: true`, una operación y está en el ledger `4858193`.
- El frontend contiene la ruta `api` + `freighter` que construye un sobre de pago USDC, lo firma y lo envía a la API. El backend `SELF_SETTLE` valida el destino, activo y monto antes de enviarlo a Horizon.

### Alcance de esta evidencia

Horizon confirma transferencias reales en **Testnet**. Su historial no demuestra por sí solo que cada una pasó por el navegador ni qué artículo se desbloqueó. No se ejecutó una compra nueva durante esta revisión. El commit `6904eba` declara una verificación manual del pago con el backend desplegado; aquí se verificó de forma independiente la existencia de los pagos y la configuración pública del backend.

`pnpm --filter @paperpay/api test:testnet` comprueba conectividad, fondeo de una cuenta de prueba y el reto 402; no ejecuta una transferencia de USDC. La ruta del facilitador OpenZeppelin no tiene liquidación real verificada en esta revisión.

El build del frontend pasa con webpack, pero emite advertencias al empaquetar `sodium-native` desde el SDK de Stellar. No se ejecutó una compra nueva en navegador durante esta revisión.

## Pendiente (actualizado al 27 de septiembre)

1. Automatizar o registrar una prueba que vincule la firma (Freighter o Pollar), el hash confirmado, la respuesta `200`, el artículo desbloqueado y el reingreso con JWT.
2. Guardar los hashes verificados en un almacén persistente, con restricción única, antes de pasar a mainnet.
3. Vincular el pago al artículo en un comprobante verificable y evitar que el panel editorial equipare cualquier transferencia USDC con una lectura.
4. Verificar el facilitador OpenZeppelin por separado si se quiere ofrecer ese modo. La ruta actual confirmada usa `SELF_SETTLE`, que espera una transacción clásica completa.
5. Reemplazar los artículos ficticios por material con licencia de distribución.
6. Probar errores de red después del envío: el pago puede quedar confirmado aunque el cliente no reciba la respuesta. El usuario debe consultar su wallet antes de reintentar.

El panel editorial calcula 98%/2% sobre los pagos entrantes, pero el código no realiza ese reparto en el ledger.
