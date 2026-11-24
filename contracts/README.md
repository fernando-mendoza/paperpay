# Contratos Soroban

## `splitter`: reparto 98% editorial / 2% PaperPay

Cobra lecturas y las reparte en la misma transacción. El USDC va directo de la cuenta que paga a la editorial (monto − comisión) y a la tesorería de PaperPay (comisión). El contrato nunca guarda fondos.

| Función | Qué hace |
| --- | --- |
| `__constructor(token, treasury, fee_bps)` | Fija el token (SAC de USDC), la tesorería y la comisión en puntos base (`200` = 2%, máximo `1000`). No cambia después: no hay admin. |
| `pay(payer, paper_id, publisher, amount) -> Split` | Pide la firma de `payer` sobre la llamada con sus argumentos, mueve `amount − fee` a `publisher` y `fee` a la tesorería, y emite el evento `paid` con `payer` como lector. |
| `pay_batch(payer, items) -> Split` | Varios artículos (`Item { paper_id, publisher, amount }`, hasta 10) con **una sola firma**. Reparte cada uno igual que `pay`, agrupa las transferencias por editorial y emite un `paid` por artículo. Es todo o nada: lista vacía, artículo repetido o inválido, o falta de saldo → no se cobra ninguno. Devuelve la suma. |
| `gift(payer, reader, paper_id, publisher, amount) -> Split` | `payer` paga y el acceso queda a nombre de `reader` (un profesor que paga la lectura de un estudiante, por ejemplo). Mismo reparto que `pay`. Solo firma `payer`. |
| `split(amount) -> Split` | El reparto que aplicaría `pay`, sin mover fondos. |
| `config() -> Config` | La configuración fijada al desplegar. |

- **Comisión:** `fee = amount × fee_bps / 10 000`, redondeada hacia abajo. La fracción de stroop queda para la editorial: de $0.50 (`5 000 000`) salen `4 900 000` para la editorial y `100 000` para la tesorería.
- **Evento `paid`:**
  - topics: `["paid", publisher, reader]`;
  - datos: `{ paper_id, payer, amount, fee }`.
  
  Es el comprobante en la red de qué artículo se pagó y quién lo puede leer. En `pay_batch` la comisión se redondea por artículo, así que un paquete cobra lo mismo que los `pay` separados.
- **Quién valida qué:** el contrato garantiza el reparto. La API sigue decidiendo el precio y la editorial de cada artículo, y valida que el evento coincida antes de dar acceso.
- **Errores:** `FeeTooHigh` (1), `InvalidAmount` (2), `InvalidPaperId` (3; vacío o de más de 64 bytes), `AmountOverflow` (4), `EmptyBatch` (5), `BatchTooLarge` (6), `DuplicatePaper` (7).

### Probar

Requiere Rust estable y el target `wasm32v1-none` (`rustup target add wasm32v1-none`).

```bash
pnpm test:contracts            # cargo test: 21 pruebas con un SAC de USDC simulado
```

### Compilar el WASM

`soroban-sdk` 29 solo compila el WASM a través de `stellar-cli` v25.2.0 o posterior (no con `cargo build`):

```bash
cd contracts && stellar contract build
```

### Despliegue en testnet

Datos completos en [`deployments/testnet.json`](deployments/testnet.json).

| Dato | Valor |
| --- | --- |
| Contrato (v0.2.0) | [`CAIMQYXIXNYSAXIRQRAAG5ZTWP26476JX4SVAMDAOXRKLRCADUDKHOR5`](https://stellar.expert/explorer/testnet/contract/CAIMQYXIXNYSAXIRQRAAG5ZTWP26476JX4SVAMDAOXRKLRCADUDKHOR5) |
| Constructor | token = SAC de USDC testnet (`CBIELTK6…`), treasury = tesorería de PaperPay (`GAB2…CWRV`), `fee_bps` = 200 |
| WASM (sha256) | `4868dd0233d3153fcd128c5e4f8ba97c7d0c544fc49a6a1174bdb68cfcf2cfa3`, igual al que se baja de la red con `stellar contract fetch` |
| Editorial demo | `GA3MOPOJ374G4APKR4RE7VPXF5CEQKER3FA3LZ2XSAARIRSZEF6QXUZI`: una sola cuenta recibe el 98% de las editoriales ficticias del catálogo |
| `pay_batch` | [`551ae39b…`](https://stellar.expert/explorer/testnet/tx/551ae39b2fd42f247cbbfc355e5dfffbb0d2fbbec2514e5a9756b92ef4b21dff): los 6 artículos de `/top` con una firma (3.00 USDC). Resultado: 2.94 a la editorial y 0.06 a la tesorería en 2 transferencias, con 6 eventos `paid` |
| `gift` | [`8dac0f7e…`](https://stellar.expert/explorer/testnet/tx/8dac0f7e574f6980da0baf49e2db3cc53191e84457e5fb38018d732dcd092753): 0.50 USDC por `stablecoins-emerging-markets` para otra cuenta, que ni siquiera existe en la red. El evento `paid` lleva a esa cuenta como `reader` |

La v0.1.0 ([`CDOUF6…BPOZ`](https://stellar.expert/explorer/testnet/contract/CDOUF6BBLKAXPT6EJAF2SOAUMKX3RASZFNHD7ORMIQZUTH4G6V66BPOZ), solo `pay`, con el [pago de prueba `cdda69b1…`](https://stellar.expert/explorer/testnet/tx/cdda69b15feb5767ddb3b8b436386a953c869e2ed30476a72028234ad130c489)) sigue en la red, pero queda reemplazada. Ninguna versión tiene admin: la cuenta que despliega es desechable y no conserva ningún poder sobre el contrato.

### Estado

Desplegado en testnet. La API y la web ya saben cobrar a través de él, probado en testnet con una API local. **Todavía no está activo** en la API desplegada: los pagos del sitio publicado siguen yendo completos a la tesorería.
