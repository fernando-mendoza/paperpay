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
| Contrato | [`CDOUF6BBLKAXPT6EJAF2SOAUMKX3RASZFNHD7ORMIQZUTH4G6V66BPOZ`](https://stellar.expert/explorer/testnet/contract/CDOUF6BBLKAXPT6EJAF2SOAUMKX3RASZFNHD7ORMIQZUTH4G6V66BPOZ) |
| Constructor | token = SAC de USDC testnet (`CBIELTK6…`), treasury = tesorería de PaperPay (`GAB2…CWRV`), `fee_bps` = 200 |
| WASM (sha256) | `0dce9d06e2c76b75d15bd57d277073e24eaa76f0c384973f72ef5e8522b40642`, igual al que se baja de la red con `stellar contract fetch` |
| Editorial demo | `GA3MOPOJ374G4APKR4RE7VPXF5CEQKER3FA3LZ2XSAARIRSZEF6QXUZI`: una sola cuenta recibe el 98% de las editoriales ficticias del catálogo |
| Pago de prueba | [`cdda69b1…`](https://stellar.expert/explorer/testnet/tx/cdda69b15feb5767ddb3b8b436386a953c869e2ed30476a72028234ad130c489): 0.50 USDC por `crispr-nanomedicine-biotech` → 0.49 a la editorial y 0.01 a la tesorería, con el evento `paid` |

La cuenta que desplegó era desechable: el contrato no tiene admin, así que esa cuenta no conserva ningún poder sobre él.

### Estado

Desplegado en testnet. **Todavía no está conectado** a la API ni a la web: los pagos del sitio siguen yendo completos a la tesorería.
