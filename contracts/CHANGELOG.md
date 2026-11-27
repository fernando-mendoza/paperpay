# Historial del contrato `splitter`

El contrato no tiene admin y no se puede actualizar: cada versión es un contrato nuevo en la red, y las anteriores siguen ahí. Las fechas están en hora de la Ciudad de México. Los datos de cada despliegue (hashes del WASM, transacciones, constructor) están en [`deployments/testnet.json`](deployments/testnet.json).

Hasta la v0.4.0, todas usan el mismo constructor: SAC de USDC de testnet, la tesorería de PaperPay y `fee_bps = 200` (2%). La v0.5.0 suma dos precios por día: `pass_day_price` (1 USDC) y `open_day_price` (5 USDC). La v0.6.0 usa el mismo constructor que la v0.5.0.

| Versión | Contrato (testnet) | Funciones nuevas | Pruebas | ¿La usa el sitio? |
| --- | --- | --- | --- | --- |
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
