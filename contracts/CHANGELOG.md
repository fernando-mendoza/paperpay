# Historial del contrato `splitter`

El contrato no tiene admin y no se puede actualizar: cada versión es un contrato nuevo en la red, y las anteriores siguen ahí. Las fechas están en hora de la Ciudad de México. Los datos de cada despliegue (hashes del WASM, transacciones, constructor) están en [`deployments/testnet.json`](deployments/testnet.json).

Todas las versiones usan el mismo constructor: SAC de USDC de testnet, la tesorería de PaperPay y `fee_bps = 200` (2%).

| Versión | Contrato (testnet) | Funciones nuevas | Pruebas | ¿La usa el sitio? |
| --- | --- | --- | --- | --- |
| [0.4.0](#040-25-nov-2026) | `CC5SIM…ADGA` | `pay_authors`, `pay_ref` | 43 | No |
| [0.3.0](#030-25-nov-2026) | `CCQRUT…HEQD` | `gift_batch`, `tip`, `refund` | 32 | No |
| [0.2.0](#020-23-nov-2026) | `CAIMQY…HOR5` | `pay_batch`, `gift` | 21 | **Sí, desde el 23 de noviembre** |
| [0.1.0](#010-23-nov-2026) | `CDOUF6…BPOZ` | `pay`, `split`, `config` | 11 | No (reemplazada) |

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
