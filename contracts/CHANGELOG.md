# Historial del contrato `splitter`

El contrato no tiene admin y no se puede actualizar: cada versión es un contrato nuevo en la red, y las anteriores siguen ahí. Las fechas están en hora de la Ciudad de México. Los datos de cada despliegue (hashes del WASM, transacciones, constructor) están en [`deployments/testnet.json`](deployments/testnet.json).

Hasta la v0.4.0, todas usan el mismo constructor: SAC de USDC de testnet, la tesorería de PaperPay y `fee_bps = 200` (2%). La v0.5.0 suma dos precios por día: `pass_day_price` (1 USDC) y `open_day_price` (5 USDC). La v0.6.0 usa el mismo constructor que la v0.5.0.

| Versión | Contrato (testnet) | Funciones nuevas | Pruebas | ¿La usa el sitio? |
| --- | --- | --- | --- | --- |
| [0.6.0](#060-26-nov-2026) | `CANRPE…QM7` | `pledge`, `release_pledges`, `refund_pledge`, `claim` (más `open_campaign`, de soporte) | 89 | No |
| [0.5.0](#050-26-nov-2026) | `CA2QHB…4RT2` | `buy_pass`, `subscribe`, `renew_subscription`, `sponsor_open` | 68 | No |
| [0.4.0](#040-25-nov-2026) | `CC5SIM…ADGA` | `pay_authors`, `pay_ref` | 43 | **Sí, desde el 25 de noviembre** (`pay`, `pay_ref` y `gift`) |
| [0.3.0](#030-25-nov-2026) | `CCQRUT…HEQD` | `gift_batch`, `tip`, `refund` | 32 | No |
| [0.2.0](#020-23-nov-2026) | `CAIMQY…HOR5` | `pay_batch`, `gift` | 21 | Del 23 al 25 de noviembre. Sigue como contrato anterior: sus regalos y lecturas valen |
| [0.1.0](#010-23-nov-2026) | `CDOUF6…BPOZ` | `pay`, `split`, `config` | 11 | No (reemplazada) |

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
