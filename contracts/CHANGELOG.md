# Historial del contrato `splitter`

El contrato no tiene admin y no se puede actualizar: cada versión es un contrato nuevo en la red, y las anteriores siguen ahí. Las fechas están en hora de la Ciudad de México. Los datos de cada despliegue (hashes del WASM, transacciones, constructor) están en [`deployments/testnet.json`](deployments/testnet.json).

Hasta la v0.4.0, todas usan el mismo constructor: SAC de USDC de testnet, la tesorería de PaperPay y `fee_bps = 200` (2%). La v0.5.0 suma dos precios por día: `pass_day_price` (1 USDC) y `open_day_price` (5 USDC). La v0.6.0 usa el mismo constructor que la v0.5.0.

| Versión | Contrato (testnet) | Funciones nuevas | Pruebas | ¿La usa el sitio? |
| --- | --- | --- | --- | --- |
| [0.20.0](#0200-27-nov-2026) | `CAK77Y…MA5K` | `extend_rental`, `accept_with_waiver`, `add_milestone`, `increase_bounty`, `pay_batch_as_agent` | 294 | No |
| [0.19.0](#0190-27-nov-2026) | `CAOZXN…FPVY` | `pay_apc`, `accept_manuscript`, `reject_manuscript`, `reclaim_apc` | 283 | No |
| [0.18.0](#0180-27-nov-2026) | `CBAXQM…SCCJ` | `rent`, `buy_out` | 267 | No |
| [0.17.0](#0170-27-nov-2026) | `CALLI4…KX2P` | `pay_from_owed` | 254 | No |
| [0.16.0](#0160-27-nov-2026) | `CCMUFZ…WBYZ` | `pay_as_agent` (más `authorize_agent` y `revoke_agent`, de soporte) | 245 | No |
| [0.15.0](#0150-27-nov-2026) | `CBFYNO…HXAV` | `fund_grant`, `release_milestone`, `cancel_grant` | 231 | No |
| [0.14.0](#0140-27-nov-2026) | `CBGHC7…ESYP` | `accept_bounty`, `submit_review`, `forfeit_stake` | 215 | No |
| [0.13.0](#0130-27-nov-2026) | `CDHBL4…3HSJ` | `lock_payment`, `unlock_payment`, `reclaim_payment` | 199 | No |
| [0.12.0](#0120-27-nov-2026) | `CCFNCZ…HIY6` | `join_club`, `approve_read`, `leave_club` (más `create_club` y `propose_read`, de soporte) | 184 | No |
| [0.11.0](#0110-27-nov-2026) | `CCMUYU…TJDZ` | `buy_global_pass`, `settle_pass`, `cancel_pass` (más `use_pass`, de soporte) | 167 | No |
| [0.10.0](#0100-27-nov-2026) | `CCGPAG…C4WH` | `open_tab`, `settle_tab`, `collect_tab`, `close_tab` (más `charge_tab`, de soporte) | 152 | No |
| [0.9.0](#090-27-nov-2026) | `CCGNE2…MYGB` | `buy_vouchers`, `redeem_voucher`, `reclaim_vouchers` | 135 | No |
| [0.8.0](#080-27-nov-2026) | `CDUFLV…PNIF` | `post_bounty`, `award_bounty`, `cancel_bounty` | 119 | No |
| [0.7.0](#070-27-nov-2026) | `CDOI4F…DK3N` | `fund_pool`, `pay_sponsored`, `withdraw_pool` | 105 | No |
| [0.6.0](#060-26-nov-2026) | `CANRPE…QM7` | `pledge`, `release_pledges`, `refund_pledge`, `claim` (más `open_campaign`, de soporte) | 89 | No |
| [0.5.0](#050-26-nov-2026) | `CA2QHB…4RT2` | `buy_pass`, `subscribe`, `renew_subscription`, `sponsor_open` | 68 | No |
| [0.4.0](#040-25-nov-2026) | `CC5SIM…ADGA` | `pay_authors`, `pay_ref` | 43 | **Sí, desde el 25 de noviembre** (`pay`, `pay_ref` y `gift`) |
| [0.3.0](#030-25-nov-2026) | `CCQRUT…HEQD` | `gift_batch`, `tip`, `refund` | 32 | No |
| [0.2.0](#020-23-nov-2026) | `CAIMQY…HOR5` | `pay_batch`, `gift` | 21 | Del 23 al 25 de noviembre. Sigue como contrato anterior: sus regalos y lecturas valen |
| [0.1.0](#010-23-nov-2026) | `CDOUF6…BPOZ` | `pay`, `split`, `config` | 11 | No (reemplazada) |

## 0.20.0 (27 nov 2026)

Cinco funciones que extienden familias que ya existían. Ninguna abre una custodia nueva ni suma errores: reusan los eventos y los errores de su familia.

- **`extend_rental(reader, publisher, paper_id, amount, period)`** (renta, v0.18.0): más tiempo para una renta vigente. Lo que se paga al extender se suma a lo que cuenta para la compra, así que `buy_out` cobra menos. El vencimiento nuevo no pasa de 7 días desde ahora y lo pagado en total sigue por debajo del precio de compra. Emite `rented`.
- **`accept_with_waiver(journal, apc_id, waived)`** (cargo de publicación, v0.19.0): la revista acepta y perdona una parte del cargo, que vuelve completa a quien lo pagó; cobra el resto con el reparto de siempre. Es la exención parcial que las revistas dan a quien no puede pagar el cargo completo. Emite `manuscript_accepted` con lo cobrado.
- **`add_milestone(funder, grant_id, amount)`** (traducción por encargo, v0.15.0): un hito más al final de un encargo abierto, hasta 10. Emite `grant_funded` con el monto agregado.
- **`increase_bounty(funder, bounty_id, amount)`** (recompensas, v0.8.0): quien publicó una recompensa la sube antes del plazo. La garantía mínima de quien la tome después se calcula sobre el total nuevo. Emite `bounty_posted` con el total nuevo.
- **`pay_batch_as_agent(agent, owner, items)`** (agente con tope, v0.16.0): el agente paga varios artículos con una sola firma. Cada uno dentro del tope por lectura y la suma dentro del presupuesto; todo o nada. Emite `paid` y `agent_paid` por artículo.
- El WASM queda en 130,465 bytes, a 607 del tope de testnet (131,072): **en este contrato ya no cabe ninguna función más**.
- Código: `0f5339d`, más tres aserciones (`eaa5751`). Versión en `d505014`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CAK77YJBWDA5DN5MBTLXNNNT4OEJROWT5DRW3WLNYZTVCAFSF6A4MA5K`](https://stellar.expert/explorer/testnet/contract/CAK77YJBWDA5DN5MBTLXNNNT4OEJROWT5DRW3WLNYZTVCAFSF6A4MA5K), WASM `037d5140…`;
  - [`extend_rental` `6b629f71…`](https://stellar.expert/explorer/testnet/tx/6b629f71c8a97b03fc05adabdd2a952e7615d48edb33161e28391935c9f70efb) (otra hora de renta por 0.10; lo pagado sube a 0.20);
  - [`accept_with_waiver` `34eeb442…`](https://stellar.expert/explorer/testnet/tx/34eeb442cd7affaced85798cb7b0a469a63019a7209bdc2f9d4c14df5b3f47a7) (de un cargo de 2.00, la revista perdona 0.50 y cobra 1.50);
  - [`add_milestone` `fbb1fb64…`](https://stellar.expert/explorer/testnet/tx/fbb1fb64f4edfffa1ff74ce89814c43452e8bd2b37b1fc1b4ff782425e17ce82) (un tercer hito de 0.30 en un encargo de dos);
  - [`increase_bounty` `a3f1b407…`](https://stellar.expert/explorer/testnet/tx/a3f1b407ef031c0ba9c6d6cc76d39b6c0294e5022bf0838297a6a728d0c937b2) (una recompensa de 1.00 sube a 1.50);
  - [`pay_batch_as_agent` `698c3d1c…`](https://stellar.expert/explorer/testnet/tx/698c3d1cd5bbc2feac5a3406837f7e5ebe7aab496b70b810a66ff9ad26cadea7) (el agente paga dos artículos, 0.50 y 0.30, con una firma).
- Pruebas: 11 nuevas, 294 en total, en los archivos de cada familia (`rent.rs`, `apc.rs`, `grant.rs`, `bounty.rs`, `agent.rs`). Se rompió el código de 48 formas a propósito y alguna prueba detectó cada una.

## 0.19.0 (27 nov 2026)

Cargo de publicación (APC) en garantía: quien envía un manuscrito no paga por publicar algo que no se publicó, y la revista sabe que el dinero existe. Es la **undécima custodia** y el primer flujo del contrato del lado de quien publica.

- **`pay_apc(author, journal, manuscript_id, amount, deadline)`:** deja el cargo en el contrato, con un plazo de hasta 90 días para que la revista decida. Emite `apc_paid`.
- **`accept_manuscript(journal, apc_id)`:** la revista acepta antes del plazo y cobra: 98% para ella y 2% a la tesorería. Emite `manuscript_accepted`.
- **`reject_manuscript(journal, apc_id)`:** la revista rechaza y el cargo vuelve completo, sin comisión, sin esperar al plazo. Emite `manuscript_rejected`.
- **`reclaim_apc(author, apc_id)`:** si la revista no decidió, desde el plazo quien pagó recupera el cargo completo (`ApcActive` antes). Emite `apc_reclaimed`.
- De lectura: `apc`.
- Errores nuevos: `NoApc` (81), `ApcClosed` (82), `ApcExpired` (83) y `ApcActive` (84). El plazo inválido reusa `InvalidDeadline` (17).
- Salió una función más de las tres planeadas: `reclaim_apc`. Sin ella, si la revista nunca decide, el cargo queda atrapado.
- Un cargo tiene una sola salida. Antes del plazo decide la revista (cobra o devuelve); desde el plazo ya no puede cobrar, y quien pagó lo recupera. Quien envió no puede "rechazarse" para sacar su dinero antes.
- Código: `a3639cb`. Versión en `7c9a150`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CAOZXNCMQ64N4X3E3UQC2RPVYVG3ULFB5AYTFMXGBMVAKARNGXDUFPVY`](https://stellar.expert/explorer/testnet/contract/CAOZXNCMQ64N4X3E3UQC2RPVYVG3ULFB5AYTFMXGBMVAKARNGXDUFPVY), WASM `30eebf3d…`;
  - [`pay_apc` `5fbbe9aa…`](https://stellar.expert/explorer/testnet/tx/5fbbe9aa9cbac442057a7a44607d4bf99bfae227ffc408be184f69124be2bdc0) (2.00 USDC en garantía por el manuscrito `ms-2026-0417`, plazo de 1 h);
  - [`accept_manuscript` `457a2cae…`](https://stellar.expert/explorer/testnet/tx/457a2cae808cb3ac2f71a6d9c36334017a9f36e3a2cfe5af9ed6b09f0d4fbdd1) (la revista cobra 1.96; 0.04 a la tesorería);
  - [`reject_manuscript` `4f27f304…`](https://stellar.expert/explorer/testnet/tx/4f27f3049a6084a752ed67cbbee5580a9a920bbe47c1445162897d5ef1d856e4) (otro cargo de 0.50: vuelve completo);
  - [`reclaim_apc` `26708910…`](https://stellar.expert/explorer/testnet/tx/26708910b580982b391ca726d0f26b73440534f1743e0ca59ff4340499a6fc98) (un tercero, con plazo de 60 s y sin decisión: quien pagó lo recupera).
- Pruebas: 16 nuevas, 283 en total, en `src/test/apc.rs`. Se rompió el código de 32 formas a propósito y alguna prueba detectó cada una.

## 0.18.0 (27 nov 2026)

Renta con opción a compra: leer un artículo por unas horas a una fracción del precio y, si sirve, quedárselo pagando solo la diferencia. **Sin custodia**: cada pago va directo de la wallet de quien lee a la editorial y a la tesorería.

- **`rent(reader, publisher, paper_id, amount, full_price, period)`:** cobra la renta (98/2) y guarda cuánto se pagó, hasta cuándo vale (`period` segundos, hasta 7 días) y el precio de compra pactado, que tiene que ser mayor que la renta. Una renta vigente no se repite; una vencida se reemplaza. Emite `rented` (no `paid`: el acceso vence).
- **`buy_out(reader, publisher, paper_id)`:** mientras la renta siga vigente, completa la compra cobrando el precio pactado menos lo ya pagado de renta. Una renta se convierte una sola vez. Emite `paid` (el acceso ya no vence) y `bought_out`.
- De lectura: `rental`.
- Errores nuevos: `NoRental` (77), `RentalExpired` (78), `RentalActive` (79) y `RentNotCheaper` (80). El periodo inválido reusa `InvalidDays` (11).
- Lo que el contrato garantiza: rentar y después comprar cuesta, entre los dos pagos, exactamente el precio de compra pactado, y la editorial y la tesorería reciben lo mismo que con una compra directa. Como en `pay`, que la renta y el precio sean los del catálogo lo valida quien da el acceso, contra los eventos.
- Código: `808b8ad`. Versión en `59b0b3f`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CBAXQMKVDTEXVVOOB7KJQJTJ2QNDQ4P3XZJYVXQB6TH3PK4DMXTISCCJ`](https://stellar.expert/explorer/testnet/contract/CBAXQMKVDTEXVVOOB7KJQJTJ2QNDQ4P3XZJYVXQB6TH3PK4DMXTISCCJ), WASM `3e79adb4…`;
  - [`rent` `6ec274be…`](https://stellar.expert/explorer/testnet/tx/6ec274be792c8a4e3dfda97a778ba7f1f2df17e550439f0ed3c58d549a52c725) (0.10 USDC por 48 h; comprar cuesta 0.50);
  - [`buy_out` `2120bcd3…`](https://stellar.expert/explorer/testnet/tx/2120bcd3918d30e50ca2597218de9be0cca936d45276ace87535d7f6ccce534b) (la compra cobra solo los 0.40 de diferencia).
- Pruebas: 13 nuevas, 267 en total, en `src/test/rent.rs`. Se rompió el código de 24 formas a propósito y alguna prueba detectó cada una.

## 0.17.0 (27 nov 2026)

Pagar con el saldo pendiente: quien ganó dinero en el contrato y todavía no puede recibir USDC lee con lo que ganó.

- **`pay_from_owed(account, publisher, paper_id, amount)`:** paga una lectura con lo que el contrato le debe a `account` (una recompensa, un hito de traducción, un cobro que quedó pendiente por no tener trustline), sin retirarlo antes: 98% a la editorial y 2% a la tesorería, desde la custodia del contrato. Si lo pendiente no alcanza, falla (`OwedTooLow`) y no cambia nada. Lo que sobra sigue pendiente y se cobra con `claim`. Emite `paid` y `paid_from_owed`.
- Error nuevo: `OwedTooLow` (76).
- Por qué no es un `pay` más: el dinero no sale de una wallet. Es el primer pago que se hace con un saldo interno del contrato, y funciona para una cuenta sin trustline de USDC.
- No cambia la custodia: gasta "lo pendiente" que el contrato ya guardaba desde la v0.6.0.
- Código: `1324c5f`, más una prueba (`2efb3b2`). Versión en `73b661f`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CALLI44DK4TUF3GCPU55V55A6CLBJS2ADZYU4JTPGJCUVN4QZCJZKX2P`](https://stellar.expert/explorer/testnet/contract/CALLI44DK4TUF3GCPU55V55A6CLBJS2ADZYU4JTPGJCUVN4QZCJZKX2P), WASM `a91c26e2…`;
  - una revisora sin trustline gana una recompensa de 1.00 ([`award_bounty` `d7f3dca1…`](https://stellar.expert/explorer/testnet/tx/d7f3dca1abb916cb4244581e4142750e997cbec4778a403e9eae19e2a734c974)): sus 0.98 quedan pendientes;
  - [`pay_from_owed` `eb6b7150…`](https://stellar.expert/explorer/testnet/tx/eb6b71507e6732bb664d1f1b7fcbc9ac67fffb20acab6b84f78dd1384db5eba4): paga 0.50 por un artículo con ese saldo (0.49 a la editorial demo, 0.01 a la tesorería); le quedan 0.48, que cobra con [`claim` `a8d2f39f…`](https://stellar.expert/explorer/testnet/tx/a8d2f39f38a91a5d11accb5c672dc8fdec90b23bb152e67bdfd9589046f6db6e).
- Pruebas: 9 nuevas, 254 en total, en `src/test/owed.rs`. Se rompió el código de 13 formas a propósito y alguna prueba detectó cada una.

## 0.16.0 (27 nov 2026)

Agente con tope: un agente de IA que investiga por alguien paga las lecturas que necesita, con el USDC de su dueño y dentro de los límites que el dueño fijó. **Sin custodia**: el contrato no guarda nada.

- **`authorize_agent(owner, agent, max_per_read, budget, days)`** (soporte, no mueve fondos): el dueño nombra a su agente, con un tope por lectura, un presupuesto total y un vencimiento (de 1 a 30 días). El presupuesto es una autorización del token al contrato (allowance): el dinero sigue en la wallet del dueño. Emite `agent_authorized`.
- **`pay_as_agent(agent, owner, publisher, paper_id, amount)`:** el agente paga una lectura **solo con su firma**: 98% a la editorial y 2% a la tesorería, directo desde la wallet del dueño. El contrato exige que sea el agente de ese dueño, que el permiso no haya vencido, y que el monto no pase del tope por lectura ni de lo que queda del presupuesto. El acceso queda a nombre del dueño. Emite `paid` y `agent_paid`.
- **`revoke_agent(owner)`** (soporte): el dueño retira el permiso y deja la autorización en cero. Emite `agent_revoked`.
- De lectura: `agent`.
- Errores nuevos: `NoAgent` (72), `AgentExpired` (73), `BudgetExhausted` (74) y `AllowanceInUse` (75). Reusa `AboveMaxPerRead` (27) e `InvalidDays` (11).
- Un agente por dueño, y no junto con una suscripción: el token guarda una sola autorización por cuenta hacia el contrato, que usan las dos cosas. Con una suscripción activa no se autoriza un agente, y si el dueño se suscribe después, el agente deja de poder pagar (`AllowanceInUse`) y la autorización de la suscripción queda intacta.
- Lo peor que puede hacer un agente comprometido: gastar el presupuesto en lecturas, al tope por lectura, hasta el vencimiento. No puede mandarse el dinero: solo sale hacia una editorial y la tesorería, con comprobante `paid` a nombre del dueño.
- Código: `6a624e0`, más una prueba (`b176ae8`). Versión en `40aa14f`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CCMUFZQR543HAJYYWMVIMA2A47VL2YVEJBRQMJA2F3UELAXAIPWEWBYZ`](https://stellar.expert/explorer/testnet/contract/CCMUFZQR543HAJYYWMVIMA2A47VL2YVEJBRQMJA2F3UELAXAIPWEWBYZ), WASM `10b35983…`;
  - [`authorize_agent` `ab4ee2e0…`](https://stellar.expert/explorer/testnet/tx/ab4ee2e08050219d68410b0bc5fd27545d90606e65da60ab00c316e46c269796) (soporte: hasta 0.50 por lectura, 1.20 en total, 1 día);
  - [`pay_as_agent` `8a1b1576…`](https://stellar.expert/explorer/testnet/tx/8a1b1576b0806ae35cf0e23cfb2ddbce2025d27f18b967a304506a286f942658) (el agente, sin USDC, paga 0.50 con el dinero de la dueña: 0.49 a la editorial demo y 0.01 a la tesorería);
  - [`revoke_agent` `03a47c0f…`](https://stellar.expert/explorer/testnet/tx/03a47c0f0ab975b06353f53dc9c81079457a7c60ea33ebd362760eccedceadc6) (soporte: el permiso se retira y la autorización queda en 0).
- Pruebas: 14 nuevas, 245 en total, en `src/test/agent.rs`. Se rompió el código de 35 formas a propósito y alguna prueba detectó cada una.

## 0.15.0 (27 nov 2026)

Traducción por encargo, pagada por hitos: alguien paga para que un artículo exista en otro idioma (o como resumen de divulgación) y el dinero sale por partes, conforme avanza el trabajo. Es la **décima custodia** y la primera que se libera en varios pagos.

- **`fund_grant(funder, translator, paper_id, milestones, deadline)`:** deja en el contrato la suma de los hitos (de 1 a 10 montos, por ejemplo borrador, revisión y versión final), con un plazo de hasta 90 días. Emite `grant_funded`.
- **`release_milestone(funder, grant_id)`:** quien paga libera el siguiente hito: 98% a quien traduce y 2% a la tesorería. Los hitos salen en orden y el último cierra el encargo. Quien traduce sin trustline no pierde su hito: queda pendiente y lo cobra con `claim`. Emite `milestone_released`.
- **`cancel_grant(funder, grant_id)`:** desde el plazo, quien paga recupera los hitos que no liberó, sin comisión (`GrantActive` antes). Lo ya pagado no vuelve. Emite `grant_cancelled`.
- De lectura: `grant`.
- Errores nuevos: `InvalidMilestones` (68), `NoGrant` (69), `GrantClosed` (70) y `GrantActive` (71). El plazo inválido reusa `InvalidDeadline` (17).
- Lo que el contrato garantiza y lo que no: garantiza a quien traduce que el dinero existe y que no se retira antes del plazo, y a quien paga, que solo sale cuando libera cada hito. Si un hito está bien hecho lo decide quien paga; quien traduce no puede liberarse un hito.
- Código: `5f0fd0f`. Versión en `3ddde33`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CBFYNOBCVMLQF3DACYYU24FLWDHJLR4ZSZZT2OJ4RKXEFYBZKF5EHXAV`](https://stellar.expert/explorer/testnet/contract/CBFYNOBCVMLQF3DACYYU24FLWDHJLR4ZSZZT2OJ4RKXEFYBZKF5EHXAV), WASM `9a5e9bdb…`;
  - [`fund_grant` `f5e843b4…`](https://stellar.expert/explorer/testnet/tx/f5e843b4ba026f570b7794dc502dd35e439227eea3a9c901738def0a932ad1d3) (2.00 USDC en tres hitos, plazo de 1 h);
  - [`release_milestone` `00f59471…`](https://stellar.expert/explorer/testnet/tx/00f594717bca572baace5dd756a564dbd6c642548cf4600954a7501e92025dde) (primer hito: la traductora no tenía trustline y lo cobró con [`claim` `2c5c01c0…`](https://stellar.expert/explorer/testnet/tx/2c5c01c00e0432c194e25e0701eef3071007ed2b87484ce8d56ff74eadf19d60));
  - [`cancel_grant` `ce133f19…`](https://stellar.expert/explorer/testnet/tx/ce133f19589d788efcaa895dff5d4dd3d3b980a0f63366c59212edafdeda5133) (otro encargo de 0.30 en dos hitos, plazo de 90 s: vuelve el hito que no se liberó).
- Pruebas: 16 nuevas, 231 en total, en `src/test/grant.rs`. Se rompió el código de 33 formas a propósito y alguna prueba detectó cada una.

## 0.14.0 (27 nov 2026)

Compromiso del revisor, sobre las recompensas de la v0.8.0: quien va a revisar o reproducir un artículo pone algo en juego. Es la **novena custodia** y el primer dinero del contrato que cambia de dueño por no cumplir un plazo.

- **`accept_bounty(reviewer, bounty_id, stake)`:** toma una recompensa abierta y deja una garantía en el contrato, de al menos el 10% de la recompensa. Una vez por cuenta; varias cuentas pueden tomar la misma. Emite `bounty_accepted`.
- **`submit_review(reviewer, bounty_id, report_hash)`:** entrega antes del plazo: el sha256 del informe queda en la red y la garantía vuelve completa. Vale aunque la recompensa ya se haya repartido. Emite `review_submitted`.
- **`forfeit_stake(bounty_id, reviewer)`:** desde el plazo, si no hubo entrega, la garantía pasa completa a quien publicó la recompensa. No pide firma: la ejecuta cualquiera y el dinero solo puede ir a esa cuenta (o queda pendiente a su nombre si no puede recibir). Emite `stake_forfeited`.
- De lectura: `stake`.
- Errores nuevos: `BountyExpired` (64), `StakeTooLow` (65), `AlreadyAccepted` (66) y `NoStake` (67). Reusa `NoBounty` (30), `BountyClosed` (31) y `BountyActive` (32).
- No cambia `post_bounty`, `award_bounty` ni `cancel_bounty`: repartir o cancelar no toca las garantías.
- Lo que el contrato garantiza y lo que no: comprueba que hubo una entrega antes del plazo y deja su hash con fecha. No juzga la calidad del informe: eso lo decide quien publicó la recompensa al repartirla. La regla es una sola: quien tomó la recompensa recupera su garantía entregando a tiempo, gane o no.
- Código: `aa59e13`. Versión en `cdd4da4`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CBGHC7PRFEOKYP6NQQB4ZIXLFHBZUDB7OJIBOJ6L73HFCZ2IR6YSESYP`](https://stellar.expert/explorer/testnet/contract/CBGHC7PRFEOKYP6NQQB4ZIXLFHBZUDB7OJIBOJ6L73HFCZ2IR6YSESYP), WASM `349fbbdc…`;
  - [`accept_bounty` `f3de4f9b…`](https://stellar.expert/explorer/testnet/tx/f3de4f9bd107dcdce313afcd7404f489c8c1a1bbb03300a3db60bd62e11b5242) (0.10 USDC de garantía por una recompensa de 1.00);
  - [`submit_review` `b2ab9434…`](https://stellar.expert/explorer/testnet/tx/b2ab943481cd813417996fe679616b8ca5186b558aa7b5393dc33839465b088f) (entrega a tiempo: la garantía vuelve);
  - [`forfeit_stake` `fb5256d1…`](https://stellar.expert/explorer/testnet/tx/fb5256d12cee842d4728eaf6aa48800a32e4d13da97653f2b8d42aa294f985ae) (otra cuenta no entregó: su garantía pasa a quien publicó).
- Pruebas: 16 nuevas, 215 en total, en `src/test/stake.rs`. Se rompió el código de 27 formas a propósito y alguna prueba detectó cada una.

## 0.13.0 (27 nov 2026)

Pago contra entrega: la editorial solo cobra si entrega la llave que abre el artículo. Es la **octava custodia** y la primera función donde el contrato comprueba la entrega por sí mismo, con un hash.

- **`lock_payment(reader, publisher, paper_id, amount, key_hash, deadline)`:** quien lee deja el pago en el contrato, atado a `key_hash` (el sha256 de la llave de 32 bytes que abre el artículo cifrado), con un plazo de hasta 7 días. Emite `payment_locked`.
- **`unlock_payment(lock_id, key)`:** quien presenta la llave antes del plazo libera el pago: 98% a la editorial y 2% a la tesorería. No pide firma: la llave es la prueba, y el dinero solo puede ir a la editorial del pago. La llave queda publicada en `payment_unlocked`, así que quien pagó la recibe en la misma transacción en que la editorial cobra. Emite también `paid`.
- **`reclaim_payment(reader, lock_id)`:** si la llave no llegó, desde el plazo quien pagó recupera todo, sin comisión (`LockActive` antes). Emite `payment_reclaimed`.
- De lectura: `lock`.
- Errores nuevos: `NoLock` (59), `LockClosed` (60), `WrongKey` (61), `LockExpired` (62) y `LockActive` (63). El plazo inválido reusa `InvalidDeadline` (17).
- Las dos salidas no se pisan: antes del plazo el dinero solo puede ir a la editorial (con la llave); desde el plazo, solo de vuelta a quien pagó.
- Lo que el contrato garantiza y lo que no: garantiza que la editorial no cobra sin publicar una llave cuyo sha256 es el pactado. No puede saber si esa llave abre de verdad el artículo: eso lo comprueba quien lee contra el archivo cifrado que recibió. Cada compra debe llevar su propia llave, porque una llave publicada ya no es secreta.
- Código: `f1dc43e`. Versión en `694aaa2`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CDHBL4S7H5QEXL7HEP47NUOGJ4JBJDVG6WHBG6GNTUMG7TIF3LT73HSJ`](https://stellar.expert/explorer/testnet/contract/CDHBL4S7H5QEXL7HEP47NUOGJ4JBJDVG6WHBG6GNTUMG7TIF3LT73HSJ), WASM `359985bc…`;
  - [`lock_payment` `85f946b1…`](https://stellar.expert/explorer/testnet/tx/85f946b15692159ba7d1830a1a722a08e5e260c9f09737be5081e1783853c24d) (0.50 USDC bloqueados, plazo de 1 h);
  - [`unlock_payment` `670863f5…`](https://stellar.expert/explorer/testnet/tx/670863f5817621ee10a7ebae8a036995f18b01016ebf4a6ca9870fdd1a5556e5) (otra cuenta presenta la llave: 0.49 a la editorial demo y 0.01 a la tesorería);
  - [`reclaim_payment` `a8943076…`](https://stellar.expert/explorer/testnet/tx/a8943076a34bc1e0a1e0a7c921ca9e957bf1b6db5a9ec8f55ebadb08aa14617b) (otro pago de 0.30, con plazo de 90 s y sin llave: vuelve completo).
- Pruebas: 15 nuevas, 199 en total, en `src/test/lock.rs`. Se rompió el código de 32 formas a propósito y alguna prueba detectó cada una.

## 0.12.0 (27 nov 2026)

Journal club: un grupo (un laboratorio, un seminario) junta una caja común y paga las lecturas que aprueba. Es la **séptima custodia** y la primera función donde el dinero se mueve por el acuerdo de varias cuentas.

- **`create_club(founder, dues, quorum)`** (soporte): define la cuota por miembro y cuántas aprobaciones hacen falta para pagar (de 2 a 10). No mueve fondos; nadie es miembro hasta que paga su cuota. Emite `club_created`.
- **`join_club(member, club_id)`:** el miembro paga su cuota a la caja del club, en el contrato. Hasta 10 miembros. Emite `club_joined`.
- **`propose_read(member, club_id, publisher, paper_id, amount)`** (soporte): un miembro propone pagar un artículo con la caja, si alcanza. Su aprobación queda contada. No mueve fondos. Emite `read_proposed`.
- **`approve_read(member, club_id, proposal_id)`:** otro miembro aprueba. La aprobación que completa el quórum paga en esa misma llamada: 98% a la editorial y 2% a la tesorería, desde la caja. Cada miembro aprueba una vez, una propuesta se paga una vez y solo cuentan las aprobaciones de quienes siguen en el club. Emite `read_approved` y, si pagó, `club_paid` con los miembros para los que queda pagado el artículo.
- **`leave_club(member, club_id)`:** quien sale se lleva una parte igual de lo que queda en la caja (la caja entre los miembros actuales). Emite `club_left`.
- De lectura: `club` y `proposal`.
- Errores nuevos: `NoClub` (50), `InvalidQuorum` (51), `ClubFull` (52), `AlreadyMember` (53), `NotMember` (54), `ClubTooLow` (55), `NoProposal` (56), `ProposalClosed` (57) y `AlreadyApproved` (58).
- Sin admin: quien crea el club no tiene ningún poder sobre la caja. El dinero solo sale por una lectura que aprobó el quórum o con cada miembro que se va.
- Por qué el quórum mínimo es 2: con 1, quien propone pagaría solo y no habría club.
- Código: `eee61ff`, más una prueba que faltaba (`924f66e`). Versión en `2b9fb29`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CCFNCZVQFRYDFRUE6CD25QVT7RRRFKMFHAU6TB7U6HWFPECLUQECHIY6`](https://stellar.expert/explorer/testnet/contract/CCFNCZVQFRYDFRUE6CD25QVT7RRRFKMFHAU6TB7U6HWFPECLUQECHIY6), WASM `87930abd…`;
  - [`join_club` `55f701c0…`](https://stellar.expert/explorer/testnet/tx/55f701c0dbd2fc38fb27af99d8a85b44373ec499a53cd9042fd3f5bb51291c6e) (la primera de tres cuotas de 0.50);
  - [`approve_read` `987b89f2…`](https://stellar.expert/explorer/testnet/tx/987b89f299eeb27a39e262745b3f487f122aa537936c8be3d0470ee91b1f4633) (la segunda aprobación completa el quórum y paga 0.50 desde la caja);
  - [`leave_club` `c25c6345…`](https://stellar.expert/explorer/testnet/tx/c25c6345ef643aed6e346a46ee0c4ed133d37b358b5d52a43e122866cdf4c1c6) (se lleva un tercio de lo que queda).
- Pruebas: 17 nuevas, 184 en total, en `src/test/club.rs`. Se rompió el código de 45 formas a propósito; dos sobrevivieron en la primera corrida y, con la prueba que faltaba, alguna prueba detecta cada una.

## 0.11.0 (27 nov 2026)

Pase PaperPay: un pase para todo el catálogo, que se reparte entre las editoriales según lo que esa persona leyó. Es la **sexta custodia** y la primera función que reparte por uso.

- **`buy_global_pass(reader, operator, amount, period)`:** quien lee paga el pase y nombra al operador (el sitio) que le anota lecturas. El dinero queda en el contrato hasta que vence (`period` segundos, hasta 31 días). Un pase a la vez. Emite `global_pass`.
- **`use_pass(operator, reader, publisher, paper_id)`** (soporte): el operador anota una lectura hecha con el pase. No mueve fondos. Solo mientras está vigente y hasta 10 editoriales distintas. Emite `pass_read`.
- **`settle_pass(reader)`:** vencido el pase, cualquiera lo liquida, sin firmas: 2% a la tesorería y el resto entre las editoriales en proporción a las lecturas de cada una. Lo que sobra por redondeo va a una sola. Si no leyó nada, todo vuelve a quien pagó, sin comisión. Emite un `pass_share` por editorial y `pass_settled`.
- **`cancel_pass(reader)`:** mientras no tenga lecturas, quien lo compró lo cancela y recupera todo. Emite `pass_cancelled`.
- De lectura: `global_pass`.
- Errores nuevos: `NoGlobalPass` (44), `PassExists` (45), `PassExpired` (46), `PassFull` (47), `PassActive` (48) y `PassUsed` (49). El operador ajeno reusa `NotOperator` (26) y la duración inválida, `InvalidDays` (11).
- En qué se distingue de `buy_pass` (v0.5.0): aquel es de una sola editorial y le paga al momento, sin custodia. Este cubre todo el catálogo y nadie cobra hasta saber qué se leyó: el dinero de cada persona va solo a las editoriales que esa persona leyó.
- Quién valida qué: el contrato no fija el precio del pase; lo decide el operador, igual que decide el precio de cada artículo en `pay`. El contrato garantiza el reparto.
- Código: `8797a3f`. Versión en `ed3cf62`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CCMUYU5MMWGSQY7XF4W3ND2CIGXZZSDTRSDN23J4756CS6IRSR5MTJDZ`](https://stellar.expert/explorer/testnet/contract/CCMUYU5MMWGSQY7XF4W3ND2CIGXZZSDTRSDN23J4756CS6IRSR5MTJDZ), WASM `13d7433d…`;
  - [`buy_global_pass` `79f0b944…`](https://stellar.expert/explorer/testnet/tx/79f0b9447ea1580e17eab14088cdf1a0be7e8adccb69407930af7c0cb3f87ab1) (2.00 USDC por 120 s);
  - [`settle_pass` `268e4d4f…`](https://stellar.expert/explorer/testnet/tx/268e4d4f9f5cef7a0b9921c93bb67716b7a21053b8b816e082d92f8b3c565fa2) (4 lecturas: tres cuartos a una editorial y un cuarto a otra);
  - [`cancel_pass` `e1284a02…`](https://stellar.expert/explorer/testnet/tx/e1284a02885e3a8a8b02745cdae567a3d0b04a073c704f89568b8a70b5f9b6ca) (otro lector, sin lecturas: recupera todo).
- Pruebas: 15 nuevas, 167 en total, en `src/test/gpass.rs`. Se rompió el código de 34 formas a propósito y alguna prueba detectó cada una.

## 0.10.0 (27 nov 2026)

Cuenta abierta: leer ahora y pagar todo junto con una firma. Es la **quinta custodia**: quien lee deja una garantía, y sus lecturas se anotan como deuda que nunca pasa de esa garantía.

- **`open_tab(reader, operator, deposit, period)`:** quien lee deja `deposit` en el contrato y nombra al operador (el sitio) que le anota lecturas. `period` son los segundos que tiene para pagar desde la primera lectura sin pagar (hasta 31 días). Una cuenta por lector. Emite `tab_opened`.
- **`charge_tab(operator, reader, publisher, paper_id, amount)`** (soporte): el operador anota una lectura como deuda con su editorial. No mueve fondos. Falla si la deuda pasaría de la garantía o de 10 editoriales (`TabFull`). Emite `charged`.
- **`settle_tab(reader)`:** quien lee paga toda la deuda con una firma: el total sale de su wallet y cada editorial cobra el 98% de lo suyo; la tesorería, el 2%. La garantía no se toca. Emite `tab_settled`.
- **`collect_tab(reader)`:** vencido el plazo de pago, cualquiera cobra la deuda de la garantía, sin firmas: el dinero solo va a quienes se les debe. Emite `tab_collected`.
- **`close_tab(reader)`:** sin deuda, quien lee recupera la garantía que queda. Emite `tab_closed`.
- De lectura: `tab`.
- Errores nuevos: `NoTab` (38), `TabExists` (39), `TabFull` (40), `NothingOwed` (41), `TabNotDue` (42) y `TabOwes` (43). El operador ajeno reusa `NotOperator` (26) y el plazo inválido, `InvalidDays` (11).
- Qué puede hacer el operador y qué no: anota deuda, hasta la garantía. No puede mover la garantía, cobrar para sí ni cerrar la cuenta.
- Cambio durante el desarrollo: el plazo de pago empezó en días (`e8a5647`) y pasó a segundos (`241e3fe`), para poder usar `collect_tab` en testnet sin esperar un día y por coherencia con los demás plazos.
- Código: `e8a5647` y `241e3fe`. Versión en `07d28e5`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CCGPAGLTT7WMVSIESZXPB2AF72I2DYZHMI42VJEPOIL726JM3HKRC4WH`](https://stellar.expert/explorer/testnet/contract/CCGPAGLTT7WMVSIESZXPB2AF72I2DYZHMI42VJEPOIL726JM3HKRC4WH), WASM `c15dcce7…`;
  - [`open_tab` `f718340c…`](https://stellar.expert/explorer/testnet/tx/f718340c316cdf6307844b276cdf6f140ed7670c5cfc881b67653f868c7f6b97) (1.00 USDC de garantía, 1 h para pagar);
  - [`settle_tab` `44490c5d…`](https://stellar.expert/explorer/testnet/tx/44490c5d758ab2e241e1538dd72a96742757de673f01677742d609ac90d11977) (0.80 de deuda con dos editoriales, pagados con una firma);
  - [`collect_tab` `8a816cc1…`](https://stellar.expert/explorer/testnet/tx/8a816cc19e1fa8edb790ff03f310d40a0178b94dee9b9cdf640b292b85eaade0) (otro lector, con plazo de 60 s y sin pagar: la deuda sale de la garantía);
  - [`close_tab` `2d1edf5e…`](https://stellar.expert/explorer/testnet/tx/2d1edf5ebcf6a6fd1e99edc0033bc63cdaf8bf4f75ffbf79fc607953427e825c) (la garantía vuelve completa).
- Pruebas: 17 nuevas, 152 en total, en `src/test/tab.rs`. Se rompió el código de 42 formas a propósito y alguna prueba detectó cada una.

## 0.9.0 (27 nov 2026)

Vales de lectura: lecturas prepagadas que se reparten como enlaces. Es la **cuarta custodia** y la primera función que se autoriza con una firma que no es de una cuenta.

- **`buy_vouchers(buyer, publisher, keys, value, expires_at)`:** paga por adelantado un vale por cada llave pública ed25519 (de 1 a 10) para el catálogo de una editorial, vigentes hasta 90 días. El contrato guarda solo la parte pública. Emite `vouchers_bought`.
- **`redeem_voucher(key, signature, reader, paper_id)`:** no pide la firma de ninguna cuenta. La autorización es la firma de la llave del vale sobre `(contrato, reader, paper_id)` en XDR: solo sirve para ese lector y ese artículo, así que cualquiera la puede enviar (el sitio, para que quien lee no necesite cuenta ni USDC). Paga 98/2 desde el contrato a la editorial del vale. Emite `paid` (con `payer` = quien compró) y `voucher_redeemed`.
- **`reclaim_vouchers(buyer, keys)`:** los vales vencidos sin usar vuelven a quien los compró, sin comisión; todo o nada. Emite `vouchers_reclaimed`.
- De lectura: `voucher`.
- Errores nuevos: `NoVoucher` (33), `VoucherSpent` (34), `VoucherExpired` (35), `VoucherActive` (36) y `VoucherExists` (37). Las listas vacías o de más de 10 llaves reusan `EmptyBatch` (5) y `BatchTooLarge` (6). Una firma inválida no da un error del contrato: la rechaza el host al verificarla.
- Una llave no se reutiliza nunca, ni después de canjeada o devuelta: así una firma vieja no sirve para un vale nuevo.
- El vale es para el catálogo de una editorial, fijada al comprarlo: quien tiene la llave elige el artículo, pero no puede desviar el dinero a otra cuenta.
- Código: `b89121c`. Versión en `6f3a384`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CCGNE2XUYN2FNRE3EBE3HSNS7Q3EW4CTISUR6RYBUWQ7ACWZGI4GMYGB`](https://stellar.expert/explorer/testnet/contract/CCGNE2XUYN2FNRE3EBE3HSNS7Q3EW4CTISUR6RYBUWQ7ACWZGI4GMYGB), WASM `9351005e…`;
  - [`buy_vouchers` `92e5cbeb…`](https://stellar.expert/explorer/testnet/tx/92e5cbebf39baaaaaa72711ba629912bc262c849fc048ddf4178c7a8d6068e1b) (2 vales de 0.50, vigentes 150 s);
  - [`redeem_voucher` `6f469d80…`](https://stellar.expert/explorer/testnet/tx/6f469d80a927a441185960043b60954c6c65a9940ba461224734accc1bc6851d) (la envía otra cuenta; el lector es una llave que no existe en la red);
  - [`reclaim_vouchers` `f9ce75a6…`](https://stellar.expert/explorer/testnet/tx/f9ce75a6f3f8cea6732958b3798600b0f93290a2a4efac22ea4e914baa50f9d5) (el vale que venció sin usarse vuelve a quien lo compró).
- Pruebas: 16 nuevas, 135 en total, en `src/test/voucher.rs`, con un vector del mensaje generado con `stellar xdr encode` para que quien firma fuera de Rust coincida byte por byte. Se rompió el código de 32 formas a propósito y alguna prueba detectó cada una.

## 0.8.0 (27 nov 2026)

Recompensas por revisar o reproducir un artículo: la **tercera custodia**. El dinero queda en el contrato con un plazo; antes del plazo solo puede salir hacia quienes ganen.

- **`post_bounty(funder, paper_id, amount, deadline)`:** deja la recompensa en garantía hasta el plazo (máximo 90 días). Emite `bounty_posted`.
- **`award_bounty(funder, bounty_id, winners)`:** quien la publicó la reparte entre 1 a 10 ganadores, con partes en puntos base; el 2% va a la tesorería. Un ganador sin trustline no bloquea a los demás: su parte queda pendiente y la cobra con `claim`. Quien publicó no puede ser ganador. Emite un `rewarded` por ganador y `bounty_awarded`.
- **`cancel_bounty(funder, bounty_id)`:** si no se repartió, quien la publicó la recupera completa, sin comisión, pero solo después del plazo (`BountyActive` antes). Emite `bounty_cancelled`.
- De lectura: `bounty`.
- Errores nuevos: `NoBounty` (30), `BountyClosed` (31) y `BountyActive` (32). Las partes inválidas reusan `InvalidShares` (8) y `TooManyAuthors` (9) de `pay_authors`, y el plazo inválido, `InvalidDeadline` (17).
- Lo que el contrato garantiza y lo que no: quién gana lo decide quien publicó la recompensa. El contrato garantiza que el dinero existe, que no se retira antes del plazo y que sale completo, una sola vez.
- El reparto por partes de `pay_authors` pasó a una función interna (`share_parts`) que usan las dos.
- Código: `a29133e`. Versión en `6c28419`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CDUFLVEYPPGHJZTUVPW45YAAB7JAEKDKVZCYAQPKQBVMWETKFB2VPNIF`](https://stellar.expert/explorer/testnet/contract/CDUFLVEYPPGHJZTUVPW45YAAB7JAEKDKVZCYAQPKQBVMWETKFB2VPNIF), WASM `d90b7c60…`;
  - [`post_bounty` `2940ac9f…`](https://stellar.expert/explorer/testnet/tx/2940ac9f0a5c8ea4b1bd83513d7f5bc3afbc61e729ea471287d96827d505f1bd) (1.00 USDC en garantía, plazo de 1 h);
  - [`award_bounty` `4d804f94…`](https://stellar.expert/explorer/testnet/tx/4d804f948709a332b3c3f5a66300573d541e2aea41b2a977ca72843a663e6f72) (60/40 entre dos ganadores; el segundo no tenía trustline y cobró con [`claim` `b592f3fe…`](https://stellar.expert/explorer/testnet/tx/b592f3fe20b9d1fdeb46eb164d11255c3c8e9a2b99a03b343823feb067178e56));
  - [`cancel_bounty` `2552af23…`](https://stellar.expert/explorer/testnet/tx/2552af234b4969c5fe093cca23ace48fe9e99765a382eb7c63d0ed2c18862184) (otra recompensa de 0.30, vencida a los 90 s: vuelve completa).
- Pruebas: 14 nuevas, 119 en total, en `src/test/bounty.rs`. Se rompió el código de 25 formas a propósito y alguna prueba detectó cada una.

## 0.7.0 (27 nov 2026)

Fondos de patrocinio: alguien deja dinero en el contrato para pagar lecturas de otros. Es la **segunda custodia**, con dos salidas y sin admin: una lectura de la editorial del fondo, o de vuelta al patrocinador.

- **`fund_pool(sponsor, publisher, operator, max_per_read, amount)`:** el patrocinador deposita para las lecturas de una editorial y nombra a un operador con un tope por lectura. Una recarga se suma al saldo y reemplaza operador y tope. Emite `pool_funded`.
- **`pay_sponsored(operator, sponsor, reader, paper_id, publisher, amount)`:** el operador paga una lectura con el fondo, 98/2 directo desde el contrato. Quien lee no paga ni firma, y puede no tener cuenta en la red. El contrato hace cumplir las reglas: solo a la editorial del fondo, hasta el tope, sin pasar del saldo y una vez por artículo y lector cada 24 h (un marcador temporal que vence solo). Emite `paid` (con `payer = sponsor`) y `sponsored`.
- **`withdraw_pool(sponsor, publisher, amount)`:** el patrocinador retira una parte o todo lo que no se usó. El operador no puede retirar. Emite `pool_withdrawn`.
- De lectura: `pool`.
- Errores nuevos: `NoPool` (25), `NotOperator` (26), `AboveMaxPerRead` (27), `PoolTooLow` (28) y `AlreadySponsored` (29).
- Si la llave del operador se filtra, el daño está acotado: no puede retirar ni pagarle a otra cuenta; solo gastar el fondo en lecturas de esa editorial.
- Código: `d7a6a12`. Versión en `c4afe8a`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CDOI4FARQOJN6AXPWZQPZN5EBO47JY275L6VRIE3DP2NC35JIAGRDK3N`](https://stellar.expert/explorer/testnet/contract/CDOI4FARQOJN6AXPWZQPZN5EBO47JY275L6VRIE3DP2NC35JIAGRDK3N), WASM `1cbe78d2…`;
  - [`fund_pool` `57cecee9…`](https://stellar.expert/explorer/testnet/tx/57cecee989c7932615b90ed8c474e523cea9607ce084ac607145977a5660ba4a) (1.50 USDC en garantía, tope de 0.50 por lectura);
  - [`pay_sponsored` `de1651fe…`](https://stellar.expert/explorer/testnet/tx/de1651fe1aa671f909a5f85dd1db85fd821618408f5258dfa66b28e0c603ad6e) (el operador paga una lectura para una llave que no existe en la red);
  - [`withdraw_pool` `ea22d0b9…`](https://stellar.expert/explorer/testnet/tx/ea22d0b94ac3ba5cd95e3a4fa01f4d3a8fe0db510a04f4488ad13ec3a6c6e3c4) (1.00 de vuelta al patrocinador).
- Pruebas: 16 nuevas, 105 en total, en `src/test/pool.rs`. Se rompió el código de 26 formas a propósito y alguna prueba detectó cada una (detalle en [`docs/QA.md`](../docs/QA.md)).

## 0.6.0 (26 nov 2026)

Campañas de acceso abierto: los lectores juntan dinero para liberar un artículo para todos. Es la **primera custodia** del contrato: los aportes quedan en garantía hasta la meta o el plazo, y lo que una cuenta no puede recibir queda pendiente a su nombre. Cada saldo tiene una sola salida (la meta, el plazo o su dueño) y no hay admin que lo pueda mover. `pay`, `gift`, los pases y las demás funciones siguen sin custodia.

- **`open_campaign(publisher, paper_id, goal, deadline)`** (soporte): la oferta pública de la editorial, con plazo de hasta 90 días. Solo firma la editorial y no mueve fondos. Emite `campaign`.
- **`pledge(backer, campaign_id, amount)`:** un aporte en garantía, mientras la campaña está abierta y sin pasar de lo que falta (`OverGoal`). El monto firmado es el que se cobra, nunca se recorta: así otro aporte entre la simulación y el envío no rompe la firma. Emite `pledged`.
- **`release_pledges(campaign_id)`:** con la meta cumplida, cualquiera la envía: 98% a la editorial y 2% a la tesorería, una sola vez. Si la editorial no puede recibir, su parte queda pendiente (`credited`). Emite `released`.
- **`refund_pledge(campaign_id, backer)`:** si la campaña venció sin la meta, cualquiera la envía y el aporte vuelve completo, sin comisión, a quien lo hizo. Emite `unpledged`.
- **`claim(account)`:** cobra lo pendiente y, con la misma firma, abre la trustline de USDC con `trust` del SAC (CAP-73). Emite `claimed`.
- De lectura: `campaign`, `pledge_of` y `owed`.
- Errores nuevos: `InvalidDeadline` (17), `NoCampaign` (18), `CampaignClosed` (19), `OverGoal` (20), `GoalNotReached` (21), `NothingToClaim` (22), `CampaignNotFailed` (23) y `NothingToRefund` (24).
- Código: `8e5e364` (`open_campaign`), `41235f1` (`pledge`), `7a889d1` (`release_pledges`), `43249e3` (`claim`), `2a63833` (`refund_pledge`) y `67f02d5` (el invariante de custodia). Versión en `36e1bec`.
- En la red (primero en una red Stellar local, después en testnet):
  - contrato [`CANRPEZT2DUNB3IP6ZDFDMAOH2H5R4AFPYRLYEMLTN4EE6O4QYSS6QM7`](https://stellar.expert/explorer/testnet/contract/CANRPEZT2DUNB3IP6ZDFDMAOH2H5R4AFPYRLYEMLTN4EE6O4QYSS6QM7), WASM `b9e66f54…`;
  - [`pledge` `2498a459…`](https://stellar.expert/explorer/testnet/tx/2498a4590283b59bb704694f140fea2471c20a7f89d2fd524329237ba97ea189) (0.60 en garantía);
  - [`release_pledges` `52040846…`](https://stellar.expert/explorer/testnet/tx/5204084632dc76ef9107b0cbdde478df45f0ecb7a9a1aea39dec86e324c74010) (la editorial sin trustline: sus 0.98 quedan pendientes);
  - [`claim` `cd62df31…`](https://stellar.expert/explorer/testnet/tx/cd62df31632cef23b99ca2d7cc8013717f947a0843392e1bdc5530d0594f19f8) (trustline nueva y cobro en la misma transacción);
  - [`refund_pledge` `acdbb11b…`](https://stellar.expert/explorer/testnet/tx/acdbb11b6d7f32ade0f92abe75c33f4d426fc982f9b1ec23a4711d1e558b4706) (campaña vencida: 0.30 de vuelta).
- Pruebas: 21 nuevas, 89 en total, incluida una que comprueba después de cada paso que el saldo del contrato es lo que está en garantía más lo pendiente. Se rompió el código de 26 formas a propósito y alguna prueba detectó cada una (detalle en [`docs/QA.md`](../docs/QA.md)).

## 0.5.0 (26 nov 2026)

Accesos con vencimiento, a precio fijo por día. El contrato guarda fechas y los términos de cada suscripción, pero **sigue sin guardar fondos**: cada cobro va directo de quien paga a la editorial (98%) y a la tesorería (2%).

- **`buy_pass(payer, reader, publisher, days, amount)`:** un pase de 1 a 31 días a todo el catálogo de una editorial. Cobra exactamente `days × pass_day_price`. Los días se suman al pase vigente, con un tope de 90 días desde ahora. Puede ser de regalo (`reader` ≠ `payer`). Emite `pass`.
- **`subscribe(reader, publisher, days, renewals, amount)`:** cobra el primer periodo y, con la misma firma, autoriza al contrato (`approve` del token) a cobrar de 1 a 12 renovaciones del mismo precio. Las renovaciones se quedan en la wallet del lector hasta cada cobro. Emite `pass` y `subscribed`.
- **`renew_subscription(reader)`:** cobra una renovación con `transfer_from`, sin firma del lector; la puede enviar cualquiera. Solo procede cuando al pase le falta un día o menos, así que nunca se cobra más de un periodo por adelantado. Emite `pass` y `renewed`.
- **`sponsor_open(sponsor, paper_id, publisher, days, amount)`:** alguien paga `days × open_day_price` para que un artículo quede abierto para todos de 1 a 31 días. Emite `opened` (no `paid`).
- De soporte (borran o solo leen): `cancel_subscription` (deja la autorización en 0; el pase pagado sigue), `pass_until`, `subscription` y `open_until`.
- Constructor: `pass_day_price` y `open_day_price`, mayores que 0.
- Errores nuevos: `InvalidDays` (11), `WrongPrice` (12), `TooLong` (13), `InvalidRenewals` (14), `NoSubscription` (15) y `RenewTooEarly` (16).
- Encontrado en una red local antes de testnet: el vencimiento de la autorización salía del ledger exacto. Es un argumento que firma el lector, y la red lo rechazaba porque avanzó entre la simulación y el envío. Ahora se redondea al día de ledgers (detalle en [`docs/QA.md`](../docs/QA.md)).
- Código: `6979bd7` (precios), `379fdb9` (`buy_pass`), `d626093` (suscripción), `94ccc7f` (`sponsor_open`) y `1afa41b` (el arreglo de la firma). Versión en `901171c`.
- En la red:
  - contrato [`CA2QHBXMF6KIBBFT5ATQYMFDP75246RQV6NC6YA55QJZX3CRVFUE4RT2`](https://stellar.expert/explorer/testnet/contract/CA2QHBXMF6KIBBFT5ATQYMFDP75246RQV6NC6YA55QJZX3CRVFUE4RT2), WASM `913ac9b4…`;
  - [`buy_pass` `747d2015…`](https://stellar.expert/explorer/testnet/tx/747d20154da627c210dde94b99a99b45042c98fb8985aa34cbbc69d4257b57a8) (pase de regalo de 1 día);
  - [`subscribe` `e4b2b08d…`](https://stellar.expert/explorer/testnet/tx/e4b2b08dc02e763d27be68bef87e3947d4446fc3cd8f903585bc320fe5f6b549) (1 día por periodo, 2 renovaciones);
  - [`renew_subscription` `3f586bef…`](https://stellar.expert/explorer/testnet/tx/3f586befa2582e47633812238990ab691facb51e22c5c47b21fb8395a18aeff5) (enviada por otra cuenta, sin firma del lector);
  - [`sponsor_open` `a49b8687…`](https://stellar.expert/explorer/testnet/tx/a49b86873d7cc33103787f9b7af5cb474469efc2494f6a4594cc437f87b313fc) (un artículo abierto 1 día).
- Pruebas: 25 nuevas, 68 en total. Se rompió el código de 27 formas a propósito y alguna prueba detectó cada una.

## 0.4.0 (25 nov 2026)

Dos formas nuevas de pagar una lectura. Las dos emiten `paid`, así que dan acceso igual que `pay`.

- **`pay_authors(payer, paper_id, authors, amount)`:** la parte de la editorial (98%) se reparte entre 1 y 10 coautores, cada uno con su porcentaje en puntos base (`Share { account, bps }`, que entre todos suman 10 000). Cada parte se redondea hacia abajo y el residuo va al primer autor, que también aparece como `publisher` en el `paid`.
- **`pay_ref(payer, paper_id, publisher, amount, referrer)`:** la editorial recibe lo mismo que en `pay`; la comisión se parte a la mitad entre quien recomendó y la tesorería. Emite además un evento nuevo, `referred`, y devuelve `RefSplit { publisher, fee, referral }`.
- Errores nuevos: `InvalidShares` (8), `TooManyAuthors` (9) y `SelfReferral` (10).
- Código: `56606da` (`pay_authors`) y `4675ae6` (`pay_ref`). Versión en `57b3a74`.
- En la red:
  - contrato [`CC5SIM7JV6TXMWABU7G3DAA3V7HROFBOBF7I5TW4JZHKAQPBFJUWADGA`](https://stellar.expert/explorer/testnet/contract/CC5SIM7JV6TXMWABU7G3DAA3V7HROFBOBF7I5TW4JZHKAQPBFJUWADGA), WASM `b97fffc8…`;
  - [`pay_authors` `23a68ac1…`](https://stellar.expert/explorer/testnet/tx/23a68ac1be070324569dcb1a39b0e9ff1a80283a4809e63112ca0a3e648a4867);
  - [`pay_ref` `ab2ca691…`](https://stellar.expert/explorer/testnet/tx/ab2ca6914b583415bf821e3223b651a4ab2d73863c827eb0634fb48752f7aa22).
- Pruebas: 11 nuevas, 43 en total. Se rompió el código de 15 formas a propósito y alguna prueba detectó cada una (detalle en [`docs/QA.md`](../docs/QA.md)).

## 0.3.0 (25 nov 2026)

Una lista de lectura de regalo, y dos movimientos que **no** dan acceso: emiten su propio evento, nunca `paid`.

- **`gift_batch(payer, reader, items)`:** `pay_batch` como regalo. Quien paga cubre hasta 10 artículos y el acceso a todos queda a nombre de otra cuenta.
- **`tip(payer, paper_id, publisher, amount)`:** una propina a la editorial, con el mismo reparto 98/2. Emite `tipped`.
- **`refund(publisher, reader, paper_id, amount)`:** la editorial le devuelve dinero a un lector y solo firma ella. Emite `refunded`. La comisión ya cobrada no vuelve.
- Por dentro: el refactor `e6e5bb8` separa `charge_batch` (lo usan `pay_batch` y `gift_batch`) y `transfer_split` (lo usan `pay`, `gift` y `tip`), para no duplicar el reparto.
- Código: `8206d34` (`gift_batch`), `cda5d9e` (`tip`) y `b28757d` (`refund`). Versión en `c47ade5`.
- En la red:
  - contrato [`CCQRUTXBSWOSOHIZIEJ637HIMJZ6CZN5ZYSMTPGBTSMFAEXY6O5QHEQD`](https://stellar.expert/explorer/testnet/contract/CCQRUTXBSWOSOHIZIEJ637HIMJZ6CZN5ZYSMTPGBTSMFAEXY6O5QHEQD), WASM `2af32e3a…`;
  - [`gift_batch` `70d30919…`](https://stellar.expert/explorer/testnet/tx/70d309192251ee041afadbbd51cd938c0fcab28fec3b258560241204544d07f9);
  - [`tip` `36e14727…`](https://stellar.expert/explorer/testnet/tx/36e14727c832a99a448d80cece792899aa117599a5b301eadde96722434ed5d5);
  - [`refund` `3bc22905…`](https://stellar.expert/explorer/testnet/tx/3bc229057ea0312ea574dfee50cde01280a7fb647fc58cc5a8d2d37e1c0910db).
- Pruebas: 11 nuevas, 32 en total, y 7 mutaciones detectadas.

## 0.2.0 (23 nov 2026)

La versión que usan el sitio y la API desde el 23 de noviembre (`SPLITTER_CONTRACT_ID`).

- **`pay_batch(payer, items)`:** hasta 10 artículos con una sola firma; todo o nada. Agrupa las transferencias por editorial y emite un `paid` por artículo.
- **`gift(payer, reader, paper_id, publisher, amount)`:** uno paga y otra cuenta lee. Con esta función funciona "Regálalo" en el sitio desde el 24 de noviembre.
- El evento `paid` pasa a llevar `reader`: topics `["paid", publisher, reader]`.
- Errores nuevos: `EmptyBatch` (5), `BatchTooLarge` (6) y `DuplicatePaper` (7).
- Código: `c4c33df`.
- En la red:
  - contrato [`CAIMQYXIXNYSAXIRQRAAG5ZTWP26476JX4SVAMDAOXRKLRCADUDKHOR5`](https://stellar.expert/explorer/testnet/contract/CAIMQYXIXNYSAXIRQRAAG5ZTWP26476JX4SVAMDAOXRKLRCADUDKHOR5), WASM `4868dd02…`;
  - [`pay_batch` `551ae39b…`](https://stellar.expert/explorer/testnet/tx/551ae39b2fd42f247cbbfc355e5dfffbb0d2fbbec2514e5a9756b92ef4b21dff);
  - [`gift` `8dac0f7e…`](https://stellar.expert/explorer/testnet/tx/8dac0f7e574f6980da0baf49e2db3cc53191e84457e5fb38018d732dcd092753).

## 0.1.0 (23 nov 2026)

Primera versión: cobra una lectura y la reparte 98% a la editorial y 2% a PaperPay en la misma transacción, sin guardar fondos.

- **`pay(payer, paper_id, publisher, amount)`:** cobra y emite `paid`.
- `split(amount)` calcula el reparto sin mover fondos y `config()` devuelve la configuración.
- Errores: `FeeTooHigh` (1), `InvalidAmount` (2), `InvalidPaperId` (3) y `AmountOverflow` (4).
- Código: `fdcdca0`.
- En la red: contrato [`CDOUF6BBLKAXPT6EJAF2SOAUMKX3RASZFNHD7ORMIQZUTH4G6V66BPOZ`](https://stellar.expert/explorer/testnet/contract/CDOUF6BBLKAXPT6EJAF2SOAUMKX3RASZFNHD7ORMIQZUTH4G6V66BPOZ) y [pago de prueba `cdda69b1…`](https://stellar.expert/explorer/testnet/tx/cdda69b15feb5767ddb3b8b436386a953c869e2ed30476a72028234ad130c489). La reemplazó la 0.2.0 antes de llegar al sitio.
