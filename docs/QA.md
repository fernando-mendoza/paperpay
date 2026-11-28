# Estado de QA

## Revisión del 27 de noviembre de 2026: contrato v0.17.0 (pagar con el saldo pendiente)

- `pnpm test:contracts`: 254 pruebas (9 nuevas, en `contracts/splitter/src/test/owed.rs`), `cargo clippy --all-targets -D warnings` y `cargo fmt --check` limpios.
  - **Sin trustline:** una cuenta G inyectada en el ledger, que ganó una recompensa, paga una lectura sin tener USDC en su wallet.
  - **Lo justo:** gastar exactamente lo pendiente lo deja en cero y borra la entrada; un stroop más falla (`OwedTooLow`) y no mueve nada.
  - **Nadie gasta lo de otro:** sin la firma de la cuenta, falla; una cuenta con USDC en su wallet pero sin nada pendiente recibe `OwedTooLow`.
  - **Editorial sin trustline:** su parte queda pendiente a su nombre y también puede leer con eso o cobrarla.
  - **Vida en la red:** pagar una parte renueva al máximo la entrada de lo que queda.
  - **Invariante de custodia:** el saldo del contrato es siempre la suma de lo pendiente de todas las cuentas.
- Mutaciones (13), con `tools/mutaciones-v0170.py` del run `2026-11-27-paperpay-contrato-v130` del hub, sobre un clon aparte del repo: todas detectadas. La lista hizo agregar una aserción (`2efb3b2`): que gastar todo no deje una entrada en cero en la red.
- Red Stellar local y testnet (`tools/usar-v0170.sh`, 27-nov 19:53 CDMX): la función usada por una revisora sin trustline y tres errores simulados antes de enviar. Saldo final del contrato: 0.
- **No probado:** no hay pantalla ni API.

## Revisión del 27 de noviembre de 2026: contrato v0.16.0 (agente con tope)

- `pnpm test:contracts`: 245 pruebas (14 nuevas, en `contracts/splitter/src/test/agent.rs`), `cargo clippy --all-targets -D warnings` y `cargo fmt --check` limpios.
  - **Solo firma el agente:** con una autorización simulada solo para esa llamada del agente (sin `mock_all_auths`), el pago sale de la wallet del dueño y el árbol de firmas no tiene sub-invocaciones.
  - **Tope y presupuesto:** un stroop arriba del tope falla (`AboveMaxPerRead`); con 2.00 de presupuesto, lo que queda exacto se puede pagar y un stroop más no (`BudgetExhausted`).
  - **Quién paga:** un extraño, la editorial y el agente de otra cuenta reciben `NoAgent`.
  - **Vencimiento:** paga un segundo antes y ya no en el segundo exacto (`AgentExpired`).
  - **Suscripciones:** con una suscripción activa no se autoriza un agente; si el dueño se suscribe después, el agente no puede gastar esa autorización y revocarlo no la toca.
  - **Sin custodia:** el contrato y el agente quedan en 0 en cada paso.
- Mutaciones (35), con `tools/mutaciones-v0160.py` del run `2026-11-27-paperpay-contrato-v130` del hub: todas detectadas. La lista hizo agregar una prueba (`b176ae8`): con una suscripción activa `revoke_agent` no llama al token, así que quitarle la firma del dueño no se habría notado. Entre las demás: autorizar sin firma o con una suscripción, el presupuesto fijado al tope, pagar como agente cualquiera, con el permiso vencido, por encima del tope, la parte de la editorial yendo al agente, y el acceso a nombre del agente.
- Red Stellar local y testnet (`tools/usar-v0160.sh`, 27-nov 16:11 CDMX): la función usada dos veces con un agente sin USDC ni trustline, las dos de soporte y cinco errores simulados antes de enviar.
- **No probado:** no hay pantalla ni API, ni un agente real firmando. Que la autorización del token venza por ledger se probó solo dentro de su vigencia (la suscripción usa el mismo cálculo).

## Revisión del 27 de noviembre de 2026: contrato v0.15.0 (traducción por encargo)

- `pnpm test:contracts`: 231 pruebas (16 nuevas, en `contracts/splitter/src/test/grant.rs`), `cargo clippy --all-targets -D warnings` y `cargo fmt --check` limpios.
  - **Hitos:** de 1 a 10, cada uno mayor que cero; una suma que desborda falla (`AmountOverflow`). Salen en orden, uno por llamada, y el último cierra el encargo.
  - **Comisión por hito:** se redondea hacia abajo en cada hito (un hito de 49 stroops no paga comisión); lo que sobra queda para quien traduce.
  - **Quién libera:** solo quien paga y con su firma. Quien traduce no puede liberarse un hito (`NoGrant`).
  - **Plazo:** no se cancela un segundo antes del plazo y sí en el segundo exacto; después del plazo todavía se puede liberar, mientras no se haya cancelado.
  - **Cancelar:** devuelve exactamente los hitos no liberados; lo ya pagado no vuelve.
  - **Quien traduce sin trustline:** su hito queda pendiente (`credited`), la tesorería cobra y después lo cobra con `claim`.
  - **Invariante de custodia:** el saldo del contrato es siempre hitos sin liberar + pendientes.
- Mutaciones (33), con `tools/mutaciones-v0150.py` del run `2026-11-27-paperpay-contrato-v130` del hub: todas detectadas. Entre ellas: encargar sin firma, sin hitos o con 11, un hito de 0, la suma desbordada sin avisar, liberar sin avanzar de hito, cerrar el encargo un hito antes o no cerrarlo, pagar el hito sin comisión o a quien paga, cancelar antes del plazo, y cancelar devolviendo también lo liberado o quedándose con un hito.
- Red Stellar local y testnet (`tools/usar-v0150.sh`, 27-nov 16:08 CDMX): las tres funciones usadas en dos encargos (uno pagado completo, otro cancelado a medias) y cuatro errores simulados antes de enviar. Saldo final del contrato: 0.
- **No probado:** no hay pantalla ni API. El contrato no sabe si la traducción se entregó: liberar cada hito es decisión de quien paga, y quien traduce no tiene forma de reclamar un hito hecho que no le liberan.

## Revisión del 27 de noviembre de 2026: contrato v0.14.0 (compromiso del revisor)

- `pnpm test:contracts`: 215 pruebas (16 nuevas, en `contracts/splitter/src/test/stake.rs`), `cargo clippy --all-targets -D warnings` y `cargo fmt --check` limpios.
  - **Mínimo:** con una recompensa de 1 USDC, 0.0999999 de garantía falla (`StakeTooLow`) y 0.10 exactos valen.
  - **Plazo:** se toma y se entrega un segundo antes del plazo, y ya no en el segundo exacto (`BountyExpired`); la garantía no se ejecuta un segundo antes (`BountyActive`) y sí en el segundo exacto. Las dos salidas nunca están abiertas a la vez.
  - **Quién:** solo entrega quien dejó la garantía y con su firma; quien publicó la recompensa no puede "entregar" por otro para quedarse con una garantía ajena (`NoStake`). Ejecutar la garantía no pide firma y el dinero solo va a quien publicó.
  - **La garantía no queda atrapada:** si la recompensa se reparte antes del plazo, quien la tomó todavía puede entregar y recuperar. Repartir y cancelar no tocan las garantías.
  - **Quien publicó sin poder recibir:** con el emisor desautorizando su cuenta, la garantía perdida queda pendiente (`credited`) y la cobra después con `claim`.
  - **Invariante de custodia:** el saldo del contrato es siempre recompensas abiertas + garantías + pendientes.
- Mutaciones (27), con `tools/mutaciones-v0140.py` del run `2026-11-27-paperpay-contrato-v130` del hub: todas detectadas. Entre ellas: tomar sin firma, una recompensa vencida o cerrada, garantía de 0 o sin mínimo, el mínimo al 1%, tomar dos veces, entregar sin firma o después del plazo, entregar sin sacar la garantía del registro, ejecutar la garantía antes del plazo o una que no existe, y la garantía perdida yendo a quien no entregó o a la tesorería.
- Red Stellar local y testnet (`tools/usar-v0140.sh`, 27-nov 16:04 CDMX): las tres funciones usadas con dos revisores (una entrega y uno no) y seis errores simulados antes de enviar. Saldo final del contrato: 0.
- **No probado:** no hay pantalla ni API. El contrato no puede saber si el informe entregado es bueno ni si el hash corresponde a un informe real: una entrega vacía recupera la garantía, y queda firmada y con fecha a nombre de quien la hizo.

## Revisión del 27 de noviembre de 2026: contrato v0.13.0 (pago contra entrega)

- `pnpm test:contracts`: 199 pruebas (15 nuevas, en `contracts/splitter/src/test/lock.rs`), `cargo clippy --all-targets -D warnings` y `cargo fmt --check` limpios.
  - **La llave es la prueba:** `unlock_payment` paga sin ninguna firma; con otra llave, con el hash presentado como llave o con la llave de otro pago, falla (`WrongKey`) y no se mueve nada.
  - **Plazo:** la llave vale un segundo antes del plazo y ya no en el segundo exacto (`LockExpired`); quien pagó no recupera un segundo antes (`LockActive`) y sí en el segundo exacto. Las dos salidas nunca están abiertas a la vez.
  - **Quién recupera:** solo quien pagó y con su firma; un extraño y la propia editorial reciben `NoLock`.
  - **Editorial sin trustline:** con una cuenta G inyectada en el ledger, su parte queda pendiente (`credited`), la tesorería cobra y después la editorial la cobra con `claim`.
  - **Una sola salida:** un pago liberado no se devuelve y uno devuelto no se libera (`LockClosed`).
  - **Invariante de custodia:** el saldo del contrato es siempre pagos bloqueados + recompensas abiertas + pendientes.
- Mutaciones (32), con `tools/mutaciones-v0130.py` del run `2026-11-27-paperpay-contrato-v130` del hub: todas detectadas. Entre ellas: bloquear sin firma, pago de 0, plazo sin tope, ids que no avanzan, cualquier llave liberando, el hash aceptado como llave, la llave valiendo después del plazo, liberar sin cerrar el pago, la editorial cobrando sin comisión o directo (una sin trustline tumbaría todo), el evento sin la llave, recuperar sin firma, antes del plazo o un pago ajeno.
- Red Stellar local y testnet (`tools/usar-v0130.sh`, 27-nov 15:54 CDMX): las tres funciones usadas y siete errores simulados antes de enviar (#59 a #63). Saldo final del contrato: 0.
- Desde esta versión, todo se compila y se prueba dentro de Docker (Rust 1.100.0 y `stellar-cli` 30.0.1). El WASM de la v0.12.0 compilado así es idéntico al desplegado (`87930abd…`).
- **No probado:** no hay pantalla ni API para el pago contra entrega, y tampoco el cifrado del artículo: el contrato solo comprueba el sha256 de la llave.

## Revisión del 27 de noviembre de 2026: contrato v0.12.0 (journal club)

- `pnpm test:contracts`: 184 pruebas (17 nuevas, en `contracts/splitter/src/test/club.rs`), `cargo clippy --all-targets -D warnings` y `cargo fmt --check` limpios.
  - **Quórum:** con quórum de 2, la segunda aprobación paga; con quórum de 3, la segunda solo se anota y la tercera paga. Quien propone no puede completar el quórum aprobando dos veces.
  - **La firma de quien aprueba no autoriza una transferencia suya:** el árbol de autorización de `approve_read` no tiene sub-invocaciones, aunque esa llamada pague desde la caja.
  - **Quien salió ya no cuenta:** si quien propuso sale del club, su aprobación deja de contar y hacen falta otras dos.
  - **Caja insuficiente o editorial sin trustline:** la aprobación falla completa y no queda anotada; la caja queda intacta.
  - **Salida:** con 1.00 en caja y tres miembros, salen 0.3333333, 0.3333333 y 0.3333334; el contrato queda en 0.
  - **Invariante de custodia:** el saldo del contrato es siempre la suma de las cajas de los clubes.
- Mutaciones (45), con `tools/mutaciones-v0120.py` del run `2026-11-27-paperpay-contrato-v070` del hub. La primera corrida dejó vivas dos (quitar la renovación del TTL al crear el club y al entrar un miembro): la prueba del TTL solo miraba un club con sus miembros ya adentro. Se agregó el caso (`924f66e`) y quedaron las 45 detectadas. Entre ellas: quórum de 1, pagar con una aprobación menos o pedir una de más, contar aprobaciones de quien ya salió, aprobar dos veces, pagar dos veces la misma propuesta, pagar sin bajar la caja, y salir llevándose toda la caja o la cuota en vez de la parte.
- Red Stellar local y testnet (`tools/usar-v0120.sh`, 27-nov 01:17 CDMX): las cinco funciones usadas con tres miembros y siete errores simulados antes de enviar. Saldo final del contrato: 0.
- **No probado:** la API y el sitio no usan todavía los clubes. Un miembro que entra tarde paga la misma cuota y, si sale, se lleva una parte igual de lo que haya: el contrato no lleva cuenta de cuánto aportó cada quien.

## Revisión del 27 de noviembre de 2026: contrato v0.11.0 (Pase PaperPay)

- `pnpm test:contracts`: 167 pruebas (15 nuevas, en `contracts/splitter/src/test/gpass.rs`), `cargo clippy --all-targets -D warnings` y `cargo fmt --check` limpios.
  - **Reparto por lecturas:** con 3 lecturas de una editorial y 1 de otra, tres cuartos y un cuarto de lo que queda después del 2%.
  - **Redondeo:** con un monto que no divide exacto entre tres editoriales, las tres reciben el piso y lo que sobra va a una sola; el contrato queda en 0.
  - **Vencimiento:** se anotan lecturas hasta un segundo antes de vencer; se liquida desde el segundo exacto.
  - **Sin lecturas:** el pase vencido vuelve completo a quien lo pagó, sin comisión; antes de vencer, lo puede cancelar.
  - **Sin firmas en `settle_pass`:** se prueba con las autorizaciones vacías.
  - **Editorial sin trustline:** su parte queda pendiente y las demás cobran.
  - **Invariante de custodia:** el saldo del contrato es siempre pases sin liquidar + pendientes.
- Mutaciones (34), con `tools/mutaciones-v0110.py` del run `2026-11-27-paperpay-contrato-v070` del hub: todas detectadas. Entre ellas: reparto parejo en vez de proporcional, dividir entre editoriales y no entre lecturas, el redondeo perdido, liquidar antes de vencer o dos veces, anotar lecturas en un pase vencido, cancelar con lecturas.
- Mutante equivalente que se quitó del código: `use_pass` renovaba el TTL del pase, pero la entrada ya vive hasta el vencimiento más la gracia desde la compra, así que esa línea no hacía nada.
- Red Stellar local y testnet (`tools/usar-v0110.sh`, 27-nov 01:06 CDMX): las cuatro funciones usadas con dos lectores y dos editoriales, y seis errores simulados antes de enviar. Saldo final del contrato: 0.
- **No probado:** la API y el sitio no usan todavía el Pase PaperPay. El operador puede anotar la misma lectura varias veces y así inclinar el reparto hacia una editorial: el contrato no lo impide, solo garantiza que el dinero va a editoriales anotadas o vuelve a quien pagó.

## Revisión del 27 de noviembre de 2026: contrato v0.10.0 (cuenta abierta)

- `pnpm test:contracts`: 152 pruebas (17 nuevas, en `contracts/splitter/src/test/tab.rs`), `cargo clippy --all-targets -D warnings` y `cargo fmt --check` limpios.
  - **La deuda nunca pasa de la garantía:** justo hasta la garantía se anota; un stroop más, no. Con 10 editoriales, la undécima no cabe, pero más deuda con una de las diez sí.
  - **Plazo de pago:** corre desde la primera lectura sin pagar y no se reinicia con las siguientes; después de pagar, una lectura nueva arranca un plazo nuevo. La garantía no se toca un segundo antes del plazo y sí en el segundo exacto.
  - **Sin firmas en `collect_tab`:** se prueba con las autorizaciones vacías.
  - **Editorial sin trustline:** su parte queda pendiente y las demás cobran.
  - **Redondeo:** la comisión se redondea por editorial y el contrato queda exactamente con la garantía.
  - **Invariante de custodia:** el saldo del contrato es siempre garantías + pendientes.
- Mutaciones (42), con `tools/mutaciones-v0100.py` del run `2026-11-27-paperpay-contrato-v070` del hub: todas detectadas, antes y después del cambio del plazo a segundos. Entre ellas: funciones sin firma, cualquiera anotando lecturas, deuda mayor que la garantía, el plazo reiniciado con cada lectura, cobrar de la garantía antes del plazo o sin bajarla, pagar sin cobrarle a quien lee y cerrar con deuda.
- Encontrado antes de desplegar: el plazo de pago iba en días, con mínimo de uno, así que `collect_tab` no se podía usar en testnet sin esperar un día. Pasó a segundos (`241e3fe`).
- Red Stellar local y testnet (`tools/usar-v0100.sh`, 27-nov 00:52 CDMX): las cinco funciones usadas con dos lectores (uno paga a tiempo, otro no) y dos editoriales, y seis errores simulados antes de enviar. Saldo final del contrato: 0.
- **No probado:** la API y el sitio no usan todavía la cuenta abierta. El operador puede anotar la misma lectura dos veces: el contrato acota el daño a la garantía, pero no lo impide.

## Revisión del 27 de noviembre de 2026: contrato v0.9.0 (vales de lectura)

- `pnpm test:contracts`: 135 pruebas (16 nuevas, en `contracts/splitter/src/test/voucher.rs`), `cargo clippy --all-targets -D warnings` y `cargo fmt --check` limpios. Las pruebas firman con `ed25519-dalek`, que entra solo como dependencia de pruebas.
  - **Sin firma de cuenta:** el canje se prueba con las autorizaciones vacías (`set_auths(&[])`): lo único que autoriza es la firma del vale.
  - **A qué está atada la firma:** falla para otro lector, otro artículo, otro vale, una firma hecha con otra llave y el mismo vale comprado en otro contrato.
  - **Vector compartido:** el mensaje que arma Rust es, byte por byte, el que da `stellar xdr encode` para los mismos datos, y el contrato acepta una firma hecha sobre esos bytes. Así un script (o el sitio) puede firmar fuera de Rust.
  - **Vencimiento:** un vale se canjea hasta un segundo antes de vencer y se devuelve desde el segundo exacto.
  - **Llaves de un solo uso:** una llave repetida en la misma compra, de otro vale, o de un vale ya canjeado o devuelto se rechaza (`VoucherExists`).
  - **Devolución todo o nada:** con un vale ajeno, usado, repetido o desconocido en la lista, no se devuelve ninguno.
  - **Invariante de custodia:** el saldo del contrato es siempre vales sin usar + fondos.
- Mutaciones (32), con `tools/mutaciones-v090.py` del run `2026-11-27-paperpay-contrato-v070` del hub: todas detectadas. Entre ellas: canje sin verificar la firma, la firma sin el lector, sin el artículo o sin el contrato, la misma llave para dos vales, cobrar un solo vale aunque sean varios, canjear vencido, devolver vales ajenos o antes de vencer.
- Red Stellar local y testnet (`tools/usar-v090.sh`, 27-nov 00:32 CDMX): llaves y firma hechas con `openssl`, mensaje armado con `stellar xdr encode`. El canje lo envió otra cuenta para un lector que no existe en la red. Saldo final del contrato: 0.
- **No probado:** la firma desde el navegador (el sitio no usa todavía los vales) y vales que se archivan después de su gracia de 30 días (habría que restaurar la entrada antes de devolverlos).

## Revisión del 27 de noviembre de 2026: contrato v0.8.0 (recompensas)

- `pnpm test:contracts`: 119 pruebas (14 nuevas, en `contracts/splitter/src/test/bounty.rs`), `cargo clippy --all-targets -D warnings` y `cargo fmt --check` limpios.
  - **Plazo:** no se cancela un segundo antes del plazo y sí en el segundo exacto; los bordes de `post_bounty` (un segundo después de ahora y justo 90 días) valen.
  - **Reparto:** 60/40 con el 2% a la tesorería; con montos que no dividen exacto, el stroop que sobra va al primer ganador y el contrato queda en 0.
  - **Ganador sin trustline:** con una cuenta G inyectada en el ledger, su parte queda pendiente (`credited`), los demás cobran y después la cobra con `claim`.
  - **Quién puede:** un extraño y el propio ganador no pueden repartir ni cancelar (`NoBounty`); quien publicó no puede ponerse como ganador (`InvalidShares`).
  - **Invariante de custodia:** el saldo del contrato es siempre recompensas abiertas + fondos + pendientes.
- Mutaciones (25), con `tools/mutaciones-v080.py` del run `2026-11-27-paperpay-contrato-v070` del hub: todas detectadas. Entre ellas: funciones sin firma, recompensa de 0, plazo sin tope, ids que no avanzan, repartir una recompensa ajena o cerrada, premiarse a sí mismo, el redondeo perdido, los ganadores cobrando directo (uno sin trustline tumbaría todo) y cancelar antes del plazo.
- Red Stellar local y testnet (`tools/usar-v080.sh`, 27-nov 00:17 CDMX): las tres funciones usadas, el segundo ganador sin trustline cobrando con `claim`, y tres errores simulados antes de enviar (#30, #31 y #32). Saldo final del contrato: 0.
- **No probado:** no hay pantalla ni API para las recompensas. Que quien publicó premie a una cuenta suya no lo puede impedir el contrato (solo impide que se ponga a sí mismo).

## Revisión del 27 de noviembre de 2026: contrato v0.7.0 (fondos de patrocinio)

- `pnpm test:contracts`: 105 pruebas (16 nuevas, en `contracts/splitter/src/test/pool.rs`), `cargo clippy --all-targets -D warnings` y `cargo fmt --check` limpios.
  - **Árboles de autorización:** `fund_pool` pide solo la firma del patrocinador (con su transferencia), `pay_sponsored` solo la del operador y `withdraw_pool` solo la del patrocinador.
  - **Reglas del fondo:** un extraño, el lector y el propio patrocinador no pueden gastar (`NotOperator`); el fondo no paga a otra editorial (`NoPool`), ni más que el tope, ni más de lo que le queda.
  - **Marcador de 24 h:** dura un día de ledgers exacto. En su último ledger la misma lectura sigue fallando; en el siguiente, ya se puede pagar otra vez.
  - **Editorial sin trustline:** con una cuenta G de verdad inyectada en el ledger, `pay_sponsored` falla completa: el fondo queda intacto, no deja pendientes ni marcador, y el patrocinador retira todo.
  - **Invariante de custodia:** después de cada paso, el saldo del contrato es lo juntado por las campañas sin liberar, más lo pendiente, más los saldos de los fondos; al final queda en 0.
- Mutaciones (26), con `tools/mutaciones.py` y `tools/mutaciones-v070.py` del run `2026-11-27-paperpay-contrato-v070` del hub: se rompió el código de 26 formas a propósito y alguna prueba detectó cada una. Entre ellas: funciones sin firma, depósito o tope en 0, recarga que pisa el saldo, cualquier cuenta como operador, sin tope, sin revisar el saldo, el marcador un ledger más corto o más largo, el retiro pagado al operador y los TTL sin renovar.
- Red Stellar local (`tools/usar-v070.sh … local`, 28 s) y testnet (`… testnet`, 60 s, 27-nov 00:06 CDMX): las tres funciones usadas y cinco errores simulados antes de enviar (#25 a #29). El lector de `pay_sponsored` es una llave que no existe en la red. Saldo final del contrato: 0.
- **No probado:** la API y el sitio no usan todavía los fondos; no hay integración ni límite por IP para lecturas patrocinadas.

## Revisión del 26 de noviembre de 2026: `/boveda`, la tarjeta de regalo (reto exprés Bóveda, en producción)

- Contrato `contracts/boveda`: 10 pruebas y 13 de 13 mutaciones detectadas. La primera corrida dejó viva "deposit devuelve el monto y no el saldo" (todas las pruebas depositaban en una bóveda vacía); se agregó la aserción.
- Web: `pnpm --filter @paperpay/web test`, 68 pruebas (5 nuevas en `test/boveda.test.ts`); `typecheck` limpio.
- E2E del servicio contra una bóveda de ensayo en testnet (`test/e2e/boveda.testnet.test.ts`, con `BOVEDA_E2E_CONTRACT`): cuenta sin activar → `NO_ACCOUNT`; Friendbot; "Ship It now" → `WRONG_CODE`; «ship it» con espacios cobra todo; otra vez → `EMPTY`.
- Navegador (Chrome headless con un Freighter simulado que responde los mensajes de `@stellar/freighter-api` 6), en `next dev` y en el export estático servido en local, celular y escritorio: sin scroll horizontal ni errores; flujo completo con cuenta nueva (Friendbot, código mal sin pedir firma, «ship it» cobra, saldo a 0). Este arnés encontró un error: el campo del código aparecía antes de saber si la cuenta existía.
- Producción: desplegado el 26-nov a las 12:01 por el script de lanzamiento, con el OK del owner (Pages `e3b6b86d`, fork `a6fee92`; la API no cambió). Contrato creado a las 12:00:05 y 5555 XLM depositados a las 12:00:13. Revisión en modo solo lectura contra [paperpay.blockchains.tf/boveda](https://paperpay.blockchains.tf/boveda): 5,555 XLM en celular y escritorio, sin scroll horizontal ni errores. En la primera pasada, el celular todavía decía "Leyendo…" a los 30 s; repetida a las 12:03, bien.
- La revisión del reto cobró a las 12:32:18 ([`e0d3e3d7…`](https://stellar.expert/explorer/testnet/tx/e0d3e3d7fbadf27ad9ff3c16c1ce67d91b147094431ded52c0fe0a3d2ffda10e)) y la vigilancia la rellenó a las 12:32:26. Fue el único cobro de la ventana.
- **No probado:** la extensión Freighter real firmando en `/boveda` (solo el simulador y la firma por SDK del e2e), ni un iPhone real.

## Revisión del 26 de noviembre de 2026: monedas 3D en la historia del pago de `/lee` (en producción)

- `pnpm --filter @paperpay/web test`: 63 pruebas (4 nuevas en `test/coin-rain.test.ts`: 49 + 1 monedas = 0.49 / 0.01, todas llegan en 0.70 cuando se confirma, los contadores nunca bajan al avanzar, la lluvia es igual en cada visita). `typecheck` limpio y export estático (`STATIC_EXPORT=1`) sin errores.
- Capturas con Chrome headless (SwiftShader) sobre el export local, con `tools/coins-shots.mjs` del run `2026-11-25-paperpay-lee-cinematico` del hub. Pantallas: 390×664, 375×548, 1440×860, más 390×664 con reduced-motion. En cada una se revisaron 4 puntos del paso de liquidación (0.30, 0.50, 0.62 y 0.90):
  - los contadores coinciden con las monedas que ya cayeron (0.00/0.00 → 0.20/0.00 → 0.37/0.01 → 0.49/0.01);
  - hay canvas de monedas, salvo con reduced-motion;
  - la tarjeta no se sale de la ventana y no hay scroll horizontal;
  - la consola no tiene errores, fuera del 404 de `/api` del servidor local.
- Lo que cambió en la historia, para que diga la verdad:
  - la ventana de Freighter ya no dice que el pago va a la tesorería: muestra la llamada `pay` al contrato (`CC5S…ADGA`);
  - el hash de la tarjeta era inventado; ahora es un pago real de 0.49 / 0.01 (`3d7b2c56…`), con enlace a stellar.expert;
  - el paso 3 cuenta el reparto: "49 a la editorial y 1 a PaperPay".
- Hallazgo previo, sin arreglar: en 375×548 la última línea del paso activo de "Cómo funciona" se corta ~14 px. Ya pasaba en producción antes de este cambio, en los cuatro pasos (medido el 26-nov contra `paperpay.blockchains.tf`).
- Producción: desplegado el 26-nov a las 06:53 con el OK del owner (Pages `5f6dd3c0`, fork `675b3a1`; la API no cambió). El mismo `coins-shots.mjs` contra [paperpay.blockchains.tf/lee](https://paperpay.blockchains.tf/lee) dio los mismos contadores en los 4 puntos de las 4 pantallas, con canvas, sin scroll horizontal y con la tarjeta dentro. Su único error de consola es el beacon de Cloudflare Web Analytics (`static.cloudflareinsights.com`), que falla con `ERR_CERT_AUTHORITY_INVALID` porque la máquina de prueba tiene el CA bundle roto (`curl` falla igual); no es del sitio. `/`, `/lee`, `/top`, `/editorial`, `/regalo` y `/api/health` dan 200.
- **No probado:** un iPhone real.

## Revisión del 26 de noviembre de 2026: contrato v0.6.0 (campañas de acceso abierto y `claim`)

- `pnpm test:contracts`: 89 pruebas (21 nuevas), `cargo clippy --all-targets -D warnings` y `cargo fmt --check` limpios.
  - **Invariante de custodia:** una prueba recorre tres campañas (cumplida, de una editorial sin trustline y fallida). Después de cada paso, el saldo del contrato es exactamente lo juntado por las campañas sin liberar más lo pendiente; al final queda en 0.
  - **Cuenta sin trustline:** las direcciones de prueba (`Address::generate`) son contratos y siempre pueden recibir. Para probar a una editorial nueva se inyecta en el ledger una cuenta G de verdad, con XLM y sin trustline: `release_pledges` deja su parte pendiente y `claim` le crea la trustline (árbol de autorización `claim → trust`).
  - **Quien aportó ya no puede recibir:** con un emisor revocable (`setup_with(true)`) se desautoriza a quien aportó; `refund_pledge` deja su aporte pendiente y lo cobra con `claim` cuando lo vuelven a autorizar.
  - **Firma de `pledge`:** una prueba firma con `mock_auths` exacto, deja que otro aporte entre antes y avanza el ledger; la firma sigue valiendo porque el monto firmado no depende del estado de la campaña.
  - **Sin cubrir en pruebas unitarias:** `mock_auths` no puede firmar como cuenta G, así que la firma real de `claim` se comprobó en la red local y en testnet (abajo).
- Mutaciones (26), con `tools/mutaciones-v060.py` del run `2026-11-26-paperpay-contrato-v060` del hub: se rompió el código de 26 formas a propósito y alguna prueba detectó cada una.
  - Campaña: sin firma de la editorial, meta 0, plazo sin tope o igual a ahora, entradas sin la gracia para devolver.
  - Aportes: sin firma, en el segundo del plazo, con la meta cumplida, sin `OverGoal`, sin sumar o sin acumular.
  - Liberación: antes de la meta, dos veces, sin marcar, sin comisión, pago directo sin pendiente, `payout` que no deja pendiente.
  - Cobro y devolución: `claim` sin firma, sin borrar o sin `trust`; devolver antes del plazo, con la meta cumplida, dos veces, sin restar o sin pendiente; lo pendiente sin TTL máximo.
  - Una revisión que no se puede romper sin que nada cambie: en `pledge`, "la campaña ya se liberó" siempre coincide con "la meta está cumplida", que ya se revisa. Se deja como defensa.
- **Red local** (`stellar container start local`) con `tools/deploy-v060.sh … local`: todo pasó a la primera. `claim` creó la trustline de una cuenta G real con CAP-73.
- **Testnet** (mismo script, preaprobado por el owner): contrato [`CANRPE…QM7`](https://stellar.expert/explorer/testnet/contract/CANRPEZT2DUNB3IP6ZDFDMAOH2H5R4AFPYRLYEMLTN4EE6O4QYSS6QM7). El WASM que se baja de la red tiene el mismo sha256 que el build (`b9e66f54…`).
  - Las 10 transacciones salieron `successful` en Horizon.
  - Efectos de Horizon: `pledge` mueve 0.60 de quien aporta al contrato; `release_pledges` paga 0.02 a la tesorería y deja los 0.98 de la editorial en el contrato; `claim` tiene `trustline_created` y el pago de 0.98 en la misma transacción; `refund_pledge` devuelve 0.30 a quien aportó.
  - Al final: saldo del contrato 0, tesorería +0.02, nada pendiente, y un segundo `claim` falla con `NothingToClaim` (#22).
- Producción no cambia: la API y el sitio siguen en v0.4.0 y no usan las campañas.

## Revisión del 26 de noviembre de 2026: contrato v0.5.0 (pases, suscripción y artículos abiertos)

- `pnpm test:contracts`: 68 pruebas (25 nuevas), `cargo clippy -D warnings` limpio. Las pruebas corren con los límites de TTL de testnet (máximo 3,110,400 ledgers) y con la hora del ledger fija, para probar vencimientos.
- Mutaciones (27): se rompió el código de 27 formas a propósito y alguna prueba detectó cada una.
  - Precios: el constructor acepta 0, `check_price` no compara el monto o acepta 0 o 32 días, `sponsor_open` cobra el precio del pase.
  - Vencimientos: los días no se suman, un acceso vencido cuenta desde el vencimiento viejo, sin tope de 90 días, TTL sin margen.
  - Firmas: `buy_pass`, `subscribe`, `cancel_subscription` y `sponsor_open` sin `require_auth`, y el pase a nombre de quien paga.
  - Suscripción: 0 renovaciones, sin tope de 150 días, allowance de más o sin el día de margen, renovación sin ventana o con la ventana corrida un segundo, sin comisión, sin descontar la renovación, sin terminar con la última, y `cancel` que deja la autorización.
  - Las dos que encontró la red local (abajo): el vencimiento de la autorización y el `approve` de `cancel` calculados con el ledger exacto.
- **Red local** (`stellar container start local`, stellar-cli 30.0.1): se desplegó el WASM (`913ac9b4…`, 28,642 bytes) y se usó cada función con el script del run `2026-11-25-paperpay-contrato-v050` del hub (`deploy-v050.sh … local`).
  - **Falla encontrada:** `subscribe` fallaba en la red con `Unauthorized function call for address` en `approve`. El vencimiento de la autorización salía del ledger del momento. Ese número va dentro de lo que firma quien lee, y la red ya había avanzado entre la simulación y el envío, así que la firma no coincidía. Las pruebas unitarias no lo veían porque `mock_all_auths` acepta cualquier argumento.
  - **Arreglo:** el vencimiento se redondea al día de ledgers, y `cancel_subscription` aprueba 0 con vencimiento 0. Hay 3 pruebas nuevas que firman con `mock_auths` exacto y avanzan el ledger antes de ejecutar; con el cálculo viejo fallan.
  - Después del arreglo, en la red local: `buy_pass` (pase de regalo), `subscribe` (1 día, 2 renovaciones), `renew_subscription` enviada por **otra cuenta sin USDC y sin firma del lector**, y `sponsor_open`. Saldos exactos: lector 1 USDC, allowance 1 USDC, contrato 0, editorial 7.84 y tesorería 0.16. Una renovación adelantada falla con `RenewTooEarly` (#16). `cancel_subscription` deja la suscripción en `null` y la allowance en 0, y una renovación posterior falla con `NoSubscription` (#15).
- **Testnet** (con el mismo script, `deploy-v050.sh … testnet`, el USDC de testnet, la tesorería de PaperPay y la editorial demo): contrato [`CA2QHB…4RT2`](https://stellar.expert/explorer/testnet/contract/CA2QHBXMF6KIBBFT5ATQYMFDP75246RQV6NC6YA55QJZX3CRVFUE4RT2). El WASM que se baja de la red tiene el mismo sha256 que el build (`913ac9b4…`).
  - Las 6 transacciones (upload, deploy, `buy_pass`, `subscribe`, `renew_subscription`, `sponsor_open`) salieron `successful` en Horizon.
  - En los efectos de Horizon de cada cobro, el USDC va directo de quien paga a la editorial demo y a la tesorería: 0.98 / 0.02 en el pase, la suscripción y la renovación, y 4.90 / 0.10 en el artículo abierto. El contrato no aparece en ningún movimiento y su saldo es 0.
  - La renovación la envió otra cuenta sin USDC; después, el lector tiene 1 USDC y la autorización quedó en 1 USDC (1 renovación pendiente). Una renovación adelantada falla con `RenewTooEarly` (#16).
- Producción no cambia: la API y el sitio siguen en v0.4.0 y no usan los pases.

## Revisión del 25 de noviembre de 2026: referidos (`pay_ref`) en la API y la web, en producción

- API: `pnpm test` con 136 pruebas en verde (13 nuevas). Cubren:
  - `pay_ref` armado y validado;
  - el referido descartado si es quien paga;
  - la vuelta a `pay` si la simulación falla;
  - el `400` con un referido inválido o en un regalo;
  - la autorización igual a la llamada;
  - los contratos anteriores en `findGift` y en `/api/health`;
  - `SPLITTER_REFERRALS` apagado.
  - Fixture: los eventos reales del `pay_ref` de testnet `ab2ca691…`.
- Web: `pnpm test` con 59 pruebas en verde (18 nuevas). Cubren:
  - la captura del `?ref=` (checksum, URL limpia, 30 días, gana el último);
  - el sobre `pay_ref` con otro referido, o `pay_ref` sin que la API lo diga;
  - el contador con el registro real de Horizon del `pay_ref`, donde la parte del referido cuenta como comisión.
  - `typecheck` limpio.
- **Testnet, contra la API local** (`CC5SIM…ADGA`, `SPLITTER_REFERRALS=true`, `test:splitter`):
  - `REFERRER=trustline`: `pay_ref` con 0.49 a la editorial, 0.005 a quien recomendó y 0.005 a la tesorería ([`1e667933…`](https://stellar.expert/explorer/testnet/tx/1e6679335b51dd92d133bf0724736fe1ed0842c730611cc013eda477c1d9b3ad)). El JWT reabre el artículo.
  - `REFERRER=sin-trustline`: la API descartó el referido y armó `pay`, con 0.49 / 0.01 ([`ff3d4515…`](https://stellar.expert/explorer/testnet/tx/ff3d45154f2696cfa0311a3099798e9d3ad317c10e4ff243a8a79f92b92e482f)).
- **Navegador** (export estático contra la API local; Puppeteer, cada caso con almacenamiento limpio):
  - `?ref=` válido en un artículo: se guarda, la URL queda sin `ref` y aparecen el aviso en el paywall y el panel, en escritorio y en la barra del celular (390×844, sin scroll horizontal);
  - `?ref=hola`: se quita de la URL y no se guarda;
  - `/lee?ref=` y luego un artículo: el aviso sigue;
  - sin referido: panel sí, aviso no;
  - sin errores de consola (fuera del `402` del paywall).
- **En producción** desde el 25-nov (CDMX; 00:18 UTC del 26), con el OK del owner:
  - **API (VPS):** `deploy-vps.sh` con 136 + 59 pruebas en verde. `/opt/paperpay` quedó en `6f0959e` y el health respondió al ~3.er segundo con la v0.2.0, igual que antes.
  - **Variables:** respaldo `/etc/paperpay/api.env.bak-20261126001804`; luego `SPLITTER_CONTRACT_ID` → v0.4.0, `SPLITTER_LEGACY_CONTRACT_IDS` = v0.2.0 y `SPLITTER_REFERRALS=true`. El restart tenía guard (si el health no mostraba v0.4.0, restauraba el respaldo). Health OK con `splitterContractIds` = [v0.4.0, v0.2.0], y el 402 publicado trae `referrals: true`.
  - **Web:** Cloudflare Pages `503f52df` (300 archivos nuevos + 56 reusados).
- **Verificado en producción:**
  - `pay_ref` real por `paperpay.blockchains.tf`, pasando por la Pages Function y la API de la VPS (`test:splitter` con `API_URL` del sitio y `REFERRER=trustline`): [`d3a130f6…`](https://stellar.expert/explorer/testnet/tx/d3a130f6aff66928b9d4c54288901d0507d61b5995918753bfe83086754d1197), 0.49 / 0.005 / 0.005. El JWT reabre el artículo y reenviar el sobre no cobra dos veces.
  - Las e2e de la web contra el sitio pasan: `pay` [`4c16b600…`](https://stellar.expert/explorer/testnet/tx/4c16b60006390d3c8d615e30fbc05bfd835f74ddc11e68b092026586729d1cbe), el memo y el regalo (`gift` en v0.4.0). El panel cuenta 68 lecturas (29 por contrato): las de la v0.2.0 siguen sumando.
    - Al correr los 3 archivos en paralelo, la prueba de `pay` falló una vez porque el pago del regalo quedó como la lectura más reciente. Sola pasó. Desde ahora `test:e2e` corre los archivos en serie.
  - `ref-shots.mjs` contra el sitio da lo mismo que en local (captura y limpieza del `?ref=`, aviso, panel, celular sin scroll horizontal). `/`, `/lee`, `/top`, `/editorial`, el artículo y `/api/health` responden 200.
  - Único error de consola: `ERR_CERT_AUTHORITY_INVALID` del beacon de `cloudflareinsights` (CA de la máquina de pruebas).
- **Rollback:**
  - Variables: `sudo cp -a /etc/paperpay/api.env.bak-20261126001804 /etc/paperpay/api.env && sudo systemctl restart paperpay-api` (vuelve a la v0.2.0 sin referidos; el código nuevo funciona igual con ese env).
  - Código: intercambiar `/opt/paperpay.old`.
  - Web: Pages `021224ea`.
- **No probado:** "Crear mi enlace" con la extensión Freighter real (el navegador de pruebas no la tiene).

## Revisión del 25 de noviembre de 2026: intro y hero cinemático de `/lee` en el celular

- Export estático (`STATIC_EXPORT=1`) servido en local; Puppeteer + Chrome con SwiftShader (WebGL).
- Celular: 390×664 y 375×548 (alto de Safari con sus barras en un iPhone 15 y un SE), y 390×844.
  - Escenas 1 → 2 → 3 en los puntos 0, 0.44 y 0.86 del scroll. En cada meseta la escena tiene opacidad 1 y las demás 0.
  - Sin encimados entre el texto y el símbolo (caja del símbolo medida con su transform), y nada se sale de la pantalla.
  - El símbolo aterriza de frente en cada escena. Con media vuelta quedaba de espaldas y la "P" se veía en espejo; se corrigió a vuelta completa.
- Intro: sus animaciones CSS congeladas con `document.getAnimations()` en 0.3, 1.0 y 2.2 s. Se ve el símbolo con las letras subiendo, luego la cortina subiendo y al final el hero completo, con la capa en `visibility: hidden`.
- Escritorio 1440×860: igual que antes. El símbolo queda con `transform: none`, las capas se separan con sus etiquetas y el texto cambia en 3 tramos.
- Reduced-motion: sin intro y con el hero de una pantalla en la escena 1. Sin JS: el hero mide una pantalla y se ve el SVG.
- Sin scroll horizontal. El único error de consola es el 404 de `/api`, porque el servidor local no tiene API y `HeroStats` falla en silencio.
- `pnpm --filter @paperpay/web typecheck` limpio y `test` con 41 pruebas en verde.
- **En producción** desde el 25-nov (Cloudflare Pages `021224ea`, código `9f3ffcc`). El mismo arnés contra `paperpay.blockchains.tf` da lo mismo que en local, y en 390×844 el dato en vivo ("60 lecturas liquidadas on-chain") entra sin encimarse. El único error de consola es el beacon de `cloudflareinsights` (CA de la máquina de pruebas, no del sitio). Rollback: `9c422ea6`.
- **No probado:** un iPhone real (Safari con GPU real, scroll táctil con inercia).

## Revisión del 25 de noviembre de 2026: contrato v0.4.0 en testnet (`pay_authors`, `pay_ref`)

- `pnpm test:contracts`: 43 pruebas (11 nuevas), `cargo clippy -D warnings` limpio. Mutaciones (15):
  - `pay_authors` (8): sin residuo al primer autor, parte en 0, parte de más del 100% (`u32::MAX + 10 001`), autor repetido, suma distinta de 100%, el máximo con error de uno, `paid` a nombre de quien paga y sin comisión.
  - `pay_ref` (7): recomendarse a sí mismo, recompensa igual a toda la comisión, redondeo hacia arriba, tesorería con la comisión entera, sin pago al referido, sin `referred`, `paid` con la comisión de la tesorería.
  - Todas las detecta alguna prueba.
- Testnet: contrato [`CC5SIM…ADGA`](https://stellar.expert/explorer/testnet/contract/CC5SIM7JV6TXMWABU7G3DAA3V7HROFBOBF7I5TW4JZHKAQPBFJUWADGA); el WASM de la red tiene el mismo sha256 que el build (`b97fffc8…`).
- [`pay_authors` `23a68ac1…`](https://stellar.expert/explorer/testnet/tx/23a68ac1be070324569dcb1a39b0e9ff1a80283a4809e63112ca0a3e648a4867) y [`pay_ref` `ab2ca691…`](https://stellar.expert/explorer/testnet/tx/ab2ca6914b583415bf821e3223b651a4ab2d73863c827eb0634fb48752f7aa22): las dos `successful`. Horizon muestra los repartos esperados: 0.245 / 0.147 / 0.098 / 0.01, y 0.49 / 0.005 / 0.005. `stellar events` muestra un `paid` (con el primer autor como `publisher`) y luego `paid` + `referred` (recompensa 50000).
- Producción no cambió: la API y el sitio siguen en v0.2.0.

## Revisión del 25 de noviembre de 2026: contrato v0.3.0 en testnet (`gift_batch`, `tip`, `refund`)

- `pnpm test:contracts`: 32 pruebas (11 nuevas), `cargo clippy -D warnings` limpio. Mutaciones (7): `gift_batch` da el acceso a quien paga, `gift_batch`/`tip` sin firma, `tip` emite `paid`, `refund` firmado por el lector, `refund` sin validar monto o id. Todas las detecta alguna prueba.
- Testnet: contrato [`CCQRUT…HEQD`](https://stellar.expert/explorer/testnet/contract/CCQRUTXBSWOSOHIZIEJ637HIMJZ6CZN5ZYSMTPGBTSMFAEXY6O5QHEQD); el WASM de la red tiene el mismo sha256 que el build (`2af32e3a…`).
- [`gift_batch` `70d30919…`](https://stellar.expert/explorer/testnet/tx/70d309192251ee041afadbbd51cd938c0fcab28fec3b258560241204544d07f9), [`tip` `36e14727…`](https://stellar.expert/explorer/testnet/tx/36e14727c832a99a448d80cece792899aa117599a5b301eadde96722434ed5d5), [`pay` `a10772f8…`](https://stellar.expert/explorer/testnet/tx/a10772f8faa8ab883fba19824caadc69412378b92396d6b6028c30745b9dea3e) y [`refund` `3bc22905…`](https://stellar.expert/explorer/testnet/tx/3bc229057ea0312ea574dfee50cde01280a7fb647fc58cc5a8d2d37e1c0910db): las cuatro `successful`. Horizon muestra los repartos esperados (0.98/0.02, 0.49/0.01, 0.49/0.01 y la devolución de 0.49). `stellar events` muestra 2 `paid` con el estudiante como `reader`, 1 `tipped`, 1 `paid` y 1 `refunded`.
- Producción no cambió: la API y el sitio siguen en v0.2.0. Ni `tipped` ni `refunded` dan acceso, porque la API solo lee `paid`.

## Revisión del 24 de noviembre de 2026: pago con memo (`?memo=`)

- Hallazgo: Stellar no admite memo en transacciones de Soroban; `@stellar/stellar-sdk` corta al armarla ("Soroban transactions cannot have a memo"). Con memo, la web paga por la transferencia clásica.
- `pnpm test`: 41 pruebas de la web (4 nuevas: el pago con memo no pide `/prepare` y arma la transferencia con el memo, un memo de más de 28 bytes no abre la wallet, la lectura de la URL).
- Testnet, código del sitio contra la API publicada (`test:e2e` de memo, lector desechable): [`0a774b39…`](https://stellar.expert/explorer/testnet/tx/0a774b39515d291e2bec0007462f656795470eebaf0a6706780aca715aa8c018), Horizon muestra `memo_type: text`, `memo: PAPERPAY-PRUEBA`; artículo entregado en 5.1 s y recibo `classic`.
- En producción (Pages `9c422ea6`, 24 de noviembre): el owner pagó desde el sitio con la extensión Freighter, con `?memo=` en la URL del artículo. Freighter mostró el memo antes de firmar. Tx [`2daeacf7…`](https://stellar.expert/explorer/testnet/tx/2daeacf77bd3ae501f2dddd8a4cb139a36c02a982c6d6d615cdc573d9fd9913e): 0.50 USDC a la tesorería, `memo: SHIP-S97KF8` (código del reto exprés "Pago relámpago" de Ship It), recibo `classic` por `crispr-nanomedicine-biotech`. El botón de regalo no lleva memo, porque va por el contrato.

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
