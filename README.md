<p align="center"><img src="apps/web/public/brand/paperpay-logo.svg" alt="PaperPay" height="56"></p>

# PaperPay

Paywall para artículos científicos: el lector paga un micropago en USDC sobre Stellar y desbloquea el artículo, sin crear cuenta ni usar tarjeta. Proyecto del track Stellar de Goya Hack 2026, construido con Next.js, Express y Stellar **Testnet**.

- **Demo:** [paperpay.blockchains.tf](https://paperpay.blockchains.tf/), en Stellar testnet y con el contrato de reparto. El sitio original del proyecto sigue en [paperpay.press](https://paperpay.press/) (Vercel), con transferencias directas a la tesorería.
- **Versión:** 3.0.0 (v1: Freighter · v2: Pollar con login social · v3: pagos por contrato Soroban, hecha en Ship It: Builder Challenge del 23 al 27 de noviembre de 2026). Notas en el [release v3.0.0](https://github.com/fernando-mendoza/paperpay/releases/tag/v3.0.0).

El catálogo contiene **52 artículos ficticios de demostración** en 10 disciplinas; sus autores, DOI, resultados y cifras no son publicaciones reales. El precio de 0.50 USDC y el reparto 98% editorial / 2% PaperPay son propuestas iniciales.

## Cómo se usa

Todo corre en **Stellar testnet**: el USDC es de prueba y no hay nada en mainnet.

1. Abre [paperpay.blockchains.tf](https://paperpay.blockchains.tf/) y elige un artículo. El resumen y la vista previa se leen sin pagar.
2. Prepara una wallet [Freighter](https://www.freighter.app/) en **Testnet**: XLM de Friendbot, la trustline de USDC y USDC de prueba de [faucet.circle.com](https://faucet.circle.com/).
3. Pulsa **Leer por $0.50 USDC**, conecta Freighter y aprueba la transacción. El contrato reparte 98% a la editorial y 2% a PaperPay en esa misma transacción.
4. El artículo se abre completo durante 24 horas en ese navegador.

Desde un artículo también puedes:

- **Regálalo:** pagas tú y compartes un enlace; quien lo abre lee sin wallet.
- **Recomiéndalo y gana:** con tu enlace `?ref=`, recibes la mitad de la comisión cuando alguien paga.

Otras pantallas: [`/editorial`](https://paperpay.blockchains.tf/editorial) muestra los pagos recibidos con su reparto, y [`/boveda`](https://paperpay.blockchains.tf/boveda) es una tarjeta de regalo con código. Para correrlo en tu máquina, ve a [Requisitos y arranque local](#requisitos-y-arranque-local).

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
| Contrato Soroban de reparto | Activo en testnet en [paperpay.blockchains.tf](https://paperpay.blockchains.tf/) y su API desde el 23 de noviembre de 2026. [`contracts/splitter`](contracts/README.md) reparte 98/2 en la misma transacción y deja un comprobante con el id del artículo. Desde el 25 de noviembre el sitio usa la v0.4.0 (`CC5SIM…ADGA`), que suma `gift_batch`, `tip`, `refund`, `pay_authors` (coautores) y `pay_ref` (referidos), todas probadas en testnet. La v0.5.0 ([`CA2QHB…4RT2`](https://stellar.expert/explorer/testnet/contract/CA2QHBXMF6KIBBFT5ATQYMFDP75246RQV6NC6YA55QJZX3CRVFUE4RT2), desplegada el 25 de noviembre) suma accesos con vencimiento, sin guardar fondos: pases al catálogo de una editorial (`buy_pass`), suscripción con renovaciones que no piden firma (`subscribe` y `renew_subscription`) y artículos abiertos para todos por un patrocinador (`sponsor_open`). Está probada en testnet, pero el sitio todavía no la usa. La v0.6.0 ([`CANRPE…QM7`](https://stellar.expert/explorer/testnet/contract/CANRPEZT2DUNB3IP6ZDFDMAOH2H5R4AFPYRLYEMLTN4EE6O4QYSS6QM7), desplegada el 26 de noviembre) suma campañas de acceso abierto: la editorial ofrece liberar un artículo por una meta (`open_campaign`), los lectores aportan en garantía (`pledge`), con la meta cumplida cobra la editorial (`release_pledges`) y, si vence el plazo, cada aporte vuelve (`refund_pledge`). Es la primera vez que el contrato guarda fondos, y cada saldo tiene una sola salida. Quien no pudo recibir su parte la cobra con `claim`, que además le abre la trustline de USDC. Probada en testnet; el sitio todavía no la usa. La v0.7.0 ([`CDOI4F…DK3N`](https://stellar.expert/explorer/testnet/contract/CDOI4FARQOJN6AXPWZQPZN5EBO47JY275L6VRIE3DP2NC35JIAGRDK3N), desplegada el 27 de noviembre) suma fondos de patrocinio: un patrocinador deposita para las lecturas de una editorial (`fund_pool`), un operador paga con ese fondo la lectura de alguien que no paga ni firma (`pay_sponsored`) y el patrocinador retira lo que no se usó (`withdraw_pool`). Probada en testnet; el sitio todavía no la usa. La v0.8.0 ([`CDUFLV…PNIF`](https://stellar.expert/explorer/testnet/contract/CDUFLVEYPPGHJZTUVPW45YAAB7JAEKDKVZCYAQPKQBVMWETKFB2VPNIF), desplegada el 27 de noviembre) suma recompensas por revisar o reproducir un artículo: alguien deja el dinero en garantía con un plazo (`post_bounty`), lo reparte entre quienes hicieron el trabajo (`award_bounty`) o lo recupera después del plazo si nadie ganó (`cancel_bounty`). Probada en testnet; el sitio todavía no la usa. La v0.9.0 ([`CCGNE2…MYGB`](https://stellar.expert/explorer/testnet/contract/CCGNE2XUYN2FNRE3EBE3HSNS7Q3EW4CTISUR6RYBUWQ7ACWZGI4GMYGB), desplegada el 27 de noviembre) suma vales de lectura: alguien paga por adelantado varios vales para el catálogo de una editorial (`buy_vouchers`); quien tiene la llave de un vale firma qué artículo quiere y cualquiera envía el canje, sin que quien lee necesite cuenta ni USDC (`redeem_voucher`); los vales vencidos vuelven a quien los compró (`reclaim_vouchers`). Probada en testnet; el sitio todavía no la usa. La v0.10.0 ([`CCGPAG…C4WH`](https://stellar.expert/explorer/testnet/contract/CCGPAGLTT7WMVSIESZXPB2AF72I2DYZHMI42VJEPOIL726JM3HKRC4WH), desplegada el 27 de noviembre) suma la cuenta abierta: quien lee deja una garantía (`open_tab`), el sitio le anota lecturas como deuda que nunca pasa de esa garantía, paga todo con una firma (`settle_tab`) y, si no paga a tiempo, cualquiera cobra la deuda de la garantía (`collect_tab`); sin deuda, recupera su garantía (`close_tab`). Probada en testnet; el sitio todavía no la usa. La v0.11.0 ([`CCMUYU…TJDZ`](https://stellar.expert/explorer/testnet/contract/CCMUYU5MMWGSQY7XF4W3ND2CIGXZZSDTRSDN23J4756CS6IRSR5MTJDZ), desplegada el 27 de noviembre) suma el Pase PaperPay: quien lee paga un pase para todo el catálogo (`buy_global_pass`), el sitio anota lo que lee y, al vencer, el dinero se reparte entre las editoriales en proporción a esas lecturas (`settle_pass`); sin lecturas, lo cancela y recupera todo (`cancel_pass`). Probada en testnet; el sitio todavía no la usa. La v0.12.0 ([`CCFNCZ…HIY6`](https://stellar.expert/explorer/testnet/contract/CCFNCZVQFRYDFRUE6CD25QVT7RRRFKMFHAU6TB7U6HWFPECLUQECHIY6), desplegada el 27 de noviembre) suma el journal club: un grupo junta una caja común con la cuota de cada miembro (`join_club`), paga un artículo cuando lo aprueba su quórum (`approve_read`) y quien sale se lleva una parte igual de lo que queda (`leave_club`). Probada en testnet; el sitio todavía no la usa. La v0.13.0 ([`CDHBL4…3HSJ`](https://stellar.expert/explorer/testnet/contract/CDHBL4S7H5QEXL7HEP47NUOGJ4JBJDVG6WHBG6GNTUMG7TIF3LT73HSJ), desplegada el 27 de noviembre) suma el pago contra entrega: quien lee deja el pago atado al hash de la llave del artículo (`lock_payment`), la editorial solo cobra si publica esa llave antes del plazo (`unlock_payment`) y, si no llega, el dinero vuelve (`reclaim_payment`). Probada en testnet; el sitio todavía no la usa. La v0.14.0 ([`CBGHC7…ESYP`](https://stellar.expert/explorer/testnet/contract/CBGHC7PRFEOKYP6NQQB4ZIXLFHBZUDB7OJIBOJ6L73HFCZ2IR6YSESYP), desplegada el 27 de noviembre) suma el compromiso del revisor: quien toma una recompensa deja una garantía (`accept_bounty`), la recupera si entrega a tiempo el hash de su informe (`submit_review`) y, si no entrega, la garantía pasa a quien publicó la recompensa (`forfeit_stake`). Probada en testnet; el sitio todavía no la usa. La v0.2.0 (`CAIMQY…HOR5`, del 23 al 25 de noviembre) quedó como contrato anterior: sus regalos y lecturas siguen valiendo. La API arma y simula `pay` (o `pay_ref`), revisa el sobre firmado y da acceso solo con el evento `paid` del artículo (`SPLITTER_CONTRACT_ID`). La web, con Freighter, pide `/prepare`, revisa el sobre antes de firmar y reenvía el mismo sobre si la confirmación tarda. El panel cuenta esas lecturas con su reparto real. Probado de punta a punta contra el sitio publicado con un lector desechable y, el 24 de noviembre, con un pago firmado en la extensión Freighter. Pollar y el sitio de Vercel siguen con transferencias completas a la tesorería. |
| Tarjeta de regalo con código (`/boveda`) | En producción desde el 26 de noviembre de 2026 en [paperpay.blockchains.tf/boveda](https://paperpay.blockchains.tf/boveda): con Freighter en testnet, quien escribe el código cobra todo el XLM guardado. El contrato es [`contracts/boveda`](contracts/README.md#boveda-tarjeta-de-regalo-con-código) ([`CAYVRB…G6VS`](https://stellar.expert/explorer/testnet/contract/CAYVRBBV5CSX5K6AHXZHI3KJS72O7OVHSHWX27MYA5HNVTP4N5OHG6VS)), sin admin: guarda el sha256 del código y entrega el saldo solo con él. Nació como la bóveda del reto exprés de Ship It (5555 XLM, código «Ship It»), que la revisión del reto cobró a las 12:32. |

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
pnpm test                               # 136 pruebas de la API y 68 de la web
pnpm build:shared && pnpm build:api
pnpm --filter @paperpay/web typecheck
pnpm build
```

Las pruebas cubren el catálogo, el 402, JWT, el modo demo y la verificación por hash con respuestas de Horizon simuladas. No ejecutan un pago real. Otras 4 pruebas de la web, de punta a punta contra testnet, se saltan si no tienen su configuración. Los contratos tienen sus propias pruebas (`cargo test` en `contracts/`): 215 del reparto y 10 de la bóveda. `pnpm --filter @paperpay/api test:testnet` comprueba Friendbot, Horizon y el 402 contra una API local, también sin pagar.

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
