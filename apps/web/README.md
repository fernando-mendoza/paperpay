# Frontend de PaperPay

```bash
cp apps/web/.env.example apps/web/.env.local
pnpm install
pnpm dev:web
```

Abre `http://localhost:3000`. La configuración de ejemplo usa la API desplegada en la VPS (`paperpay-api.blockchains.tf`) y la wallet Freighter. Para una demo sin backend ni wallet, cambia a `NEXT_PUBLIC_DATA_SOURCE=mock` y `NEXT_PUBLIC_WALLET=mock`. Los 52 artículos, sus autores y DOI son datos ficticios.

En producción ([paperpay.press](https://paperpay.press/)) el workflow `.github/workflows/deploy-web-vercel.yml` despliega en cada push a `main` que toque `apps/web`, `packages/shared` o las dependencias; las variables se configuran en Vercel. La réplica [paperpay.blockchains.tf](https://paperpay.blockchains.tf/) es un export estático en Cloudflare Pages (`STATIC_EXPORT=1`); cómo se compila y publica está en [deploy/cloudflare-pages/README.md](../../deploy/cloudflare-pages/README.md).

## Landings de campaña

Páginas para anuncios, separadas del catálogo (`/`). Viven en el grupo `src/app/(landing)/`, que monta el scroll suave (Lenis) y `MotionConfig`, y salen con `noindex, follow` para no competir con el catálogo en buscadores. No usan el `Navbar` del sitio: traen un header con un solo CTA (`src/components/landing/`).

- `/lee`: la historia del pago. Hero con el símbolo en 3D (three.js, `HeroLogo3D` + `logo3d-scene.ts`, cargado aparte): en escritorio la sección se queda fija y, al bajar, las tres "P" se separan como hojas con una etiqueta por paso del pago y se vuelven a juntar con un giro; en móvil se abre al cruzar la pantalla. Sin WebGL o sin JS se ve el SVG del logo. Después, manifiesto que se enciende con el scroll y "Cómo funciona" fijo en pantalla, donde cada paso cambia una ventana de navegador hecha en HTML (resumen → firma en Freighter → liquidación → artículo abierto). Después, un carrusel de las 10 disciplinas que avanza de lado con el scroll; cada tarjeta lleva al catálogo filtrado (`/?disciplina=<nombre>`, que `HomeView` lee al cargar).
- `/top`: seis artículos elegidos a mano, uno por disciplina. Abanico de tres en el hero y, al bajar, tarjetas que se apilan con el scroll; cada una tiene su botón directo a la página de pago del artículo (`/papers/<id>`). La lista (ids y la razón de leer cada uno) vive en `src/lib/top-picks.ts`; si un id no existe en el catálogo, el build falla.

Las dos llevan "Los números salen de la red" (`LiveNumbers`): lecturas liquidadas leídas en vivo de Horizon (pagos USDC a la tesorería y, si la API anuncia el contrato de reparto, sus lecturas), artículos y disciplinas del catálogo y ~5 s por pago, con enlace a la tesorería en stellar.expert. El hero y esta sección comparten una sola consulta (`getEditorialStatsShared`); si Horizon falla, el número de lecturas queda en "—".

Con `prefers-reduced-motion` no hay Lenis ni movimiento, y sin JavaScript el contenido se muestra igual (`<noscript>` en el layout del grupo).

## Modos de datos y wallet

`NEXT_PUBLIC_DATA_SOURCE=mock` (el valor por defecto si la variable no existe) funciona sin API y nunca transfiere fondos. `NEXT_PUBLIC_DATA_SOURCE=api` usa el backend: Next reenvía `/api/*` a `API_PROXY_TARGET`. Para un backend local, usa `API_PROXY_TARGET=http://localhost:4000` y reinicia Next. `NEXT_PUBLIC_API_URL` permite llamar directamente desde el navegador si CORS está configurado.

`NEXT_PUBLIC_WALLET=mock` usa una dirección ficticia. En modo `api`, envía `unsigned-demo-signature`; la API solo la acepta si `DEMO_PAYMENTS=true` fuera de producción. `NEXT_PUBLIC_WALLET=freighter` conecta la extensión, comprueba Testnet, construye un pago clásico de USDC, pide la firma con Freighter y envía el XDR firmado al backend. Esta ruta requiere `SELF_SETTLE=true` en la API y saldo XLM y USDC de Testnet, además de la trustline de USDC. El lector paga la comisión de red de esa transacción.

Si el `402` trae `extra.splitter` (API con `SPLITTER_CONTRACT_ID`), Freighter paga por el [contrato de reparto](../../contracts/README.md) en vez del pago clásico (`src/services/splitterPayment.ts`):

1. Revisa en Horizon que la wallet existe y tiene la trustline y el USDC del precio, para avisar qué falta.
2. Pide a la API la invocación `pay` ya simulada (`POST /api/papers/:id/prepare` con `{ payer }`).
3. Comprueba que el sobre es exactamente el pago del `402`: contrato, función `pay`, artículo, editorial, monto, wallet y autorizaciones. Si algo no coincide, no abre Freighter.
4. Freighter firma y el sobre viaja en `payment-signature`, igual que el pago clásico. Esta ruta no depende de `SELF_SETTLE`.

Pollar sigue con el pago clásico.

### Regalar un artículo

Con Freighter y una API que anuncia `extra.splitter.giftUrl`, el artículo muestra "Regálalo" (`components/GiftPanel.tsx`, `services/gift.ts`): quien regala paga 0.50 USDC por el contrato (`gift`) y recibe un enlace `/regalo#<txHash>.<secreto>`. Quien lo abre no necesita wallet: `/regalo` (`views/GiftClaimView.tsx`) firma con la llave del enlace, la API entrega el acceso de 24 h y el sitio abre el artículo. Quien paga no recibe el artículo.

- La llave se crea en el navegador y se guarda en `localStorage` **antes** de abrir Freighter, así que cerrar la pestaña después de pagar no pierde el enlace. El panel lista los regalos enviados desde ese navegador.
- El sobre se revisa igual que en `pay`, más el destinatario: si `/prepare` devuelve otra cosa, Freighter no se abre.
- Si la red no confirma a tiempo, "Reintentar" reenvía el mismo sobre.
- Quien tiene el enlace tiene el regalo. Pollar no puede regalar (no usa el contrato).

El modo de wallet también se puede elegir con `?wallet=freighter`, `?wallet=pollar` o `?wallet=mock`; la selección persiste durante la sesión. Un pago real requiere el modo `api` con Freighter o Pollar. El recibo enlaza al explorador si el backend devuelve un hash de transacción real.

El JWT y el recibo se guardan en `localStorage` por artículo. El servidor valida el JWT en cada lectura y caduca a las 24 horas. El catálogo lee esos accesos y marca como "Comprado" los artículos con acceso vigente en ese navegador. Si después de firmar la API responde `504` (la red no confirmó a tiempo), sigue en `503` tras dos reenvíos o no responde, el sitio guarda el sobre firmado en `sessionStorage` y muestra «Tu pago sigue en camino». «Reintentar» reenvía ese mismo sobre: la red lo aplica una sola vez, así que no hay doble cobro. Si la red lo rechaza, se olvida y el siguiente intento firma uno nuevo. Dos límites:

- Por contrato, el sobre pendiente viaja con el comprobante que devolvió `/prepare` (`claim`). Con él, la API entrega el acceso hasta 24 h después de confirmado; sin él, solo 5 minutos. El comprobante vive en `sessionStorage`: si se cierra la pestaña, se pierde.
- En el pago clásico, reenviar un sobre ya aplicado responde con error aunque el pago se haya hecho. Revisa el historial de la wallet antes de pagar otra vez.

## Estado verificado y límites

El 25 de septiembre de 2026 Horizon mostraba transferencias de 0.50 USDC a la tesorería y el backend desplegado respondía `SELF_SETTLE` en `/api/health`; consulta [QA](../../docs/QA.md). El historial público confirma las transferencias, pero no identifica qué artículo desbloqueó cada una. Para el pago por contrato sí hay una prueba de punta a punta (`test:e2e`, abajo). El pago clásico no la tiene.

El panel editorial lee desde Horizon los pagos que llegan a la tesorería y los separa en dos tipos:

- **Por contrato:** muestra el 98/2 real de cada transacción. `pay` y `gift` cuentan 1 lectura; `pay_batch`, una por artículo. Solo cuenta el contrato que anuncia `/api/health`.
- **Transferencia directa** (pago clásico y Pollar): llega completa a la tesorería y el 98/2 es **un cálculo visual**. Estas transferencias no necesariamente equivalen a lecturas.

`src/lib/soroban.ts` lee esas llamadas de los parámetros de Horizon sin cargar el SDK de Stellar.

Para revisar estados de la interfaz durante desarrollo, agrega `?debug=1` a un artículo. El panel de depuración no aparece en builds de producción.

Verificación estática: `pnpm --filter @paperpay/web typecheck`. Compilación: `pnpm --filter @paperpay/web build`.

Pruebas:

- `pnpm --filter @paperpay/web test`: 37 pruebas sin red, del pago por contrato (sobre que no coincide, saldo, reenvío del pendiente), del regalo (llave guardada antes de firmar, sobre con otro destinatario, reenvío, firma del reclamo) y del panel. El panel se prueba con registros reales de la tesorería en Horizon (`test/fixtures`).
- `PAPERPAY_E2E_API_URL=http://127.0.0.1:4011 pnpm --filter @paperpay/web test:e2e` **sí paga 0.50 USDC de prueba en testnet**. Corre el código del sitio contra una API con el contrato activo; Freighter se simula con la llave de un lector desechable. Recorre 402 → `/prepare` → firma → artículo, reabre el artículo con el JWT y comprueba en Horizon y en el panel la lectura por contrato. Con `PAPERPAY_E2E_API_URL=https://paperpay.blockchains.tf` pasa por la Pages Function y la API publicadas. La misma orden corre también el regalo (`test/e2e/gift.testnet.test.ts`, otros 0.50 USDC de prueba): `gift` → enlace → otro navegador abre el artículo sin wallet, quien paga sigue sin acceso y una llave ajena no lo abre. Con `PAPERPAY_E2E_GIFT_ORIGIN=http://localhost:3000` imprime además el enlace para abrirlo a mano (con `next dev`, usa `localhost`: Next bloquea sus recursos de desarrollo si se abre por `127.0.0.1`).

## Pollar (v2)

Pollar permite pagar con login social (Google o email): crea la wallet del lector, activa USDC y patrocina la comisión. A diferencia de Freighter, **Pollar firma y envía la transacción**, así que el backend la verifica por hash en Horizon (`POST /api/papers/:id/verify`).

Pollar se activa cuando existe `NEXT_PUBLIC_POLLAR_PUBLISHABLE_KEY`. Con la key, el paywall abre un selector (`components/WalletChoiceModal.tsx`) que ofrece Pollar y Freighter; sin ella, el flujo de Freighter queda igual que en v1. Piezas principales:

- `services/walletMode.ts`: modo de wallet (`mock` | `freighter` | `pollar`) y disponibilidad de Pollar.
- `components/PollarProviderGate.tsx`: monta `PollarProvider` solo si hay key.
- `hooks/usePollarCheckout.ts` y `services/pollarPayment.ts`: login, pago con `sendPayment` y verificación por hash.

Si la API no confirma el pago de inmediato (Horizon puede tardar en indexarlo), el frontend guarda el hash pendiente y reintenta la verificación durante unos 2 minutos, para no cobrar dos veces.
