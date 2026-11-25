# Estado de QA

## Revisión del 24 de noviembre de 2026: pago con memo (`?memo=`)

- Hallazgo: Stellar no admite memo en transacciones de Soroban; `@stellar/stellar-sdk` corta al armarla ("Soroban transactions cannot have a memo"). Con memo, la web paga por la transferencia clásica.
- `pnpm test`: 41 pruebas de la web (4 nuevas: el pago con memo no pide `/prepare` y arma la transferencia con el memo, un memo de más de 28 bytes no abre la wallet, la lectura de la URL).
- Testnet, código del sitio contra la API publicada (`test:e2e` de memo, lector desechable): [`0a774b39…`](https://stellar.expert/explorer/testnet/tx/0a774b39515d291e2bec0007462f656795470eebaf0a6706780aca715aa8c018), Horizon muestra `memo_type: text`, `memo: PAPERPAY-PRUEBA`; artículo entregado en 5.1 s y recibo `classic`.

## Revisión del 24 de noviembre de 2026: regalo por enlace en producción

Uno paga por el contrato (`gift`) y otro lee con un enlace, sin wallet. Detalle en `apps/api/README.md` ("Regalo por enlace").

**Comprobado:**

- `pnpm test`: 122 pruebas de la API y 37 de la web. Las nuevas cubren `/prepare` con `reader`, la validación del sobre `gift` (y que no pase como `pay`), el comprobante a nombre de quien recibe, el reclamo (firma de la llave, firma vencida, otra llave, pago normal, búsqueda en la red), la migración de recibos v1 → v2 y, en la web, la llave guardada antes de firmar y el reenvío del sobre pendiente.
- Mutaciones: quitar la verificación de la firma, no comparar `reader`, aceptar un pago normal como regalo, quitar la ventana de 5 min, no comparar `reader` en el comprobante, aceptar un sobre con otro destinatario en la web, no guardar la llave antes de firmar y no borrar la llave de un regalo sin firmar rompen 2, 1, 1, 1, 2, 1, 1 y 1 pruebas.
- Testnet contra una API local (`test:e2e`): regalo [`2df3f2c3…`](https://stellar.expert/explorer/testnet/tx/2df3f2c311a000ab664082a7b36e64f310de0e38230de52edf9434bbf34204fd) confirmado en 4.3 s. Quien pagó siguió sin acceso; otro navegador abrió el artículo solo con el enlace; el recibo trae `payer` y `reader`; una llave ajena recibió "Ese enlace no es el de este regalo". El panel lo cuenta como lectura por contrato (0.49 / 0.01).
- `findGift` contra el RPC de testnet encuentra este regalo y el del 23 de noviembre (`8dac0f7e…`), y devuelve `null` para un pago normal.
- La migración v1 → v2 sobre una copia de un archivo real de la revisión anterior: `user_version` 1 → 2, columna `reader`, el recibo queda a nombre de quien pagó.

- Navegador (Chrome headless contra `next dev` y la API local): el panel "Regálalo" aparece bajo la tarjeta de pago en escritorio y bajo la vista previa en móvil, sin scroll horizontal. Con un regalo real de testnet ([`c40d8910…`](https://stellar.expert/explorer/testnet/tx/c40d8910b86d3f79fc0bcbd9efe3121e1600bfc2a32d25ae19d4de68604834c2)), el enlace abierto en un navegador limpio mostró el título del artículo; "Abrir mi regalo" llevó al artículo completo con "Acceso activo". Un enlace roto muestra su aviso.

- Producción (API `82f8418` en la VPS y Pages `e05744d8`, 24 de noviembre): al arrancar, la API migró el archivo de recibos a v2 (el recibo existente quedó con `reader = payer`). El `402` publicado anuncia `giftUrl`, `/regalo` responde con `noindex` y `no-referrer`. `test:e2e` contra [paperpay.blockchains.tf](https://paperpay.blockchains.tf/): regalo [`3fe887ee…`](https://stellar.expert/explorer/testnet/tx/3fe887ee932f4e260267dc80b4a2a7c5d31ecf1d6b776a7eea7bc6931d517f7e) (5.2 s, abierto con el enlace, llave ajena rechazada) y pago normal [`5e8b4c03…`](https://stellar.expert/explorer/testnet/tx/5e8b4c03a526be339338976f66861a50886730622bd280fe9283f16e69c22e8b) (4.7 s). El recibo publicado distingue los dos.

- Después del despliegue (fork, sin desplegar; solo importa tras un rollback): la migración de recibos completa `reader` en cada arranque. Antes, si la API volvía a `4beee9e` y luego a una versión nueva, los recibos escritos entretanto quedaban sin `reader`. Prueba nueva (123 en total); sin el arreglo, 2 fallan.

**No comprobado:** el lado de quien regala con la extensión Freighter real (la firma se simuló con la llave del lector).

## Revisión del 24 de noviembre de 2026: recibos persistentes en producción

Cada pago aceptado deja un recibo en SQLite (`node:sqlite`): hash → artículo, wallet, método, monto y hora del ledger.

**Comprobado:**

- `pnpm test`: 98 pruebas de la API y 28 de la web. Las nuevas cubren el recibo de Pollar con la hora de Horizon, el mismo hash para otro artículo después de un reinicio (409 sin consultar Horizon), el reintento fuera de la ventana de 5 minutos (409), un hash del contrato usado en `/verify` (409), cerrar y reabrir el archivo, el endpoint `GET /api/receipts/:txHash` y que un fallo al escribir no le quite el artículo al lector.
- Mutaciones: quitar la ventana, pisar recibos, no consultar el almacén o aceptar un hash del contrato en `/verify` rompen 1, 1, 5 y 1 pruebas.
- Testnet con una API local escribiendo a un archivo: pago por contrato [`00e4300b…`](https://stellar.expert/explorer/testnet/tx/00e4300bbd639d2886c67147cb344aeacd4a77b2329f031ac5de7e8208ecea4c) (0.49 / 0.01). Otra API arrancada después sobre el mismo archivo devolvió el recibo con su artículo, wallet, método `splitter` y hora del ledger.

- Producción (API `4beee9e` en la VPS desde el 24 de noviembre): la unidad nueva crea `/var/lib/paperpay/receipts.db`. `test:e2e` contra [paperpay.blockchains.tf](https://paperpay.blockchains.tf/) pagó por contrato ([`a31fe80f…`](https://stellar.expert/explorer/testnet/tx/a31fe80f1883ee9022b3d08e5cfbb7f28be34e937a335fe71f4320ae4272d963), 4.1 s) y `GET /api/receipts/a31fe80f…` publicado devolvió el recibo. Después de reiniciar el servicio, la API arrancó con 1 recibo y lo siguió sirviendo. Archivo: 56 KB; RAM del servicio recién arrancado: 38 MB.

**No comprobado:** un pago real de Pollar con el recibo en disco.

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

## Pendiente (actualizado al 24 de noviembre)

Hecho desde la lista del 27 de septiembre:

- **Prueba de punta a punta** (firma → hash → `200` → artículo → reingreso con JWT): `test:e2e` para el pago por contrato y el regalo, también contra el sitio publicado. El pago clásico y Pollar no la tienen.
- **Almacén persistente con restricción única:** recibos en SQLite con el hash como clave primaria (en producción desde el 24 de noviembre).
- **Pago ligado al artículo en un comprobante verificable:** el evento `paid` del contrato lleva el artículo y el lector; el panel separa lecturas por contrato de transferencias directas y el reparto 98/2 ocurre en el ledger.
- **Errores de red después del envío** (pago por contrato y regalo): el sitio guarda el sobre firmado y lo reenvía; con el comprobante de `/prepare`, el acceso se recupera hasta 24 h después.

Sigue pendiente:

1. Respaldo de los recibos fuera de la VPS (pospuesto por decisión del owner). Sin él, perder el disco pierde los recibos de Pollar y los regalos de más de ~7 días.
2. Un pago real de Pollar con el recibo en disco, y regalar con la extensión Freighter real.
3. Verificar el facilitador OpenZeppelin si se quiere ofrecer ese modo.
4. Reemplazar los artículos ficticios por material con licencia de distribución (entregable 3 del SOW, con mainnet).
5. Pollar y el sitio de Vercel siguen con transferencias directas: su 98/2 en el panel es un cálculo visual.
