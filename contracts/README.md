# Contratos Soroban

Dos contratos: [`splitter`](#splitter-reparto-98-editorial--2-paperpay) (los pagos de lectura) y [`boveda`](#boveda-tarjeta-de-regalo-con-código) (la tarjeta de regalo de `/boveda`).

## `splitter`: reparto 98% editorial / 2% PaperPay

Cobra lecturas y las reparte en la misma transacción. El USDC va directo de la cuenta que paga a la editorial (monto − comisión) y a la tesorería de PaperPay (comisión): los pagos no pasan por el contrato. Las excepciones guardan dinero en garantía: las campañas de acceso abierto (desde la v0.6.0), los fondos de patrocinio (v0.7.0), las recompensas (v0.8.0), los vales de lectura (v0.9.0), la garantía de las cuentas abiertas (v0.10.0), los Pases PaperPay sin liquidar (v0.11.0), las cajas de los journal clubs (v0.12.0), los pagos contra entrega sin cobrar (v0.13.0), las garantías de los revisores (v0.14.0) y los hitos sin liberar de los encargos (v0.15.0), abajo.

Desde la v0.3.0 también regala listas de lectura (`gift_batch`), cobra propinas (`tip`) y registra devoluciones (`refund`). Las propinas y las devoluciones emiten su propio evento, nunca `paid`, así que no dan acceso a ningún artículo.

Historial de versiones, con su código y sus transacciones: [`CHANGELOG.md`](CHANGELOG.md).

La v0.4.0 suma dos formas de pagar una lectura: `pay_authors` (la parte de la editorial se reparte entre coautores) y `pay_ref` (quien recomendó la lectura se lleva la mitad de la comisión). Las dos emiten `paid`, así que dan acceso igual que `pay`.

La v0.5.0 suma **accesos con vencimiento**, a precio fijo por día:

- `buy_pass`: un pase de N días a todo el catálogo de una editorial;
- `subscribe` y `renew_subscription`: el mismo pase, renovado solo con una autorización (allowance) del token que quien lee firma una vez;
- `sponsor_open`: alguien paga para que un artículo quede abierto para todos N días.

El contrato guarda fechas (hasta cuándo vale cada pase o artículo abierto) y los términos de cada suscripción, pero **sigue sin guardar fondos**: cada cobro va directo de quien paga a la editorial y a la tesorería, también la renovación.

La v0.6.0 suma **campañas de acceso abierto**: los lectores juntan dinero para liberar un artículo para todos. Es la **primera custodia** del contrato:

- `open_campaign`: la editorial ofrece liberar un artículo si los aportes llegan a una meta antes de un plazo;
- `pledge`: un aporte, que queda **guardado en el contrato** en garantía;
- `release_pledges`: con la meta cumplida, la editorial cobra el 98% y la tesorería el 2%;
- `refund_pledge`: si vence el plazo sin llegar, cada aporte vuelve completo a quien lo hizo;
- `claim`: quien no pudo recibir su parte (por ejemplo, una editorial nueva sin trustline de USDC) la cobra después, y la misma firma le abre la trustline.

Cada saldo en custodia tiene **una sola salida**, sin admin que lo pueda mover: la meta (`release_pledges`), el plazo (`refund_pledge`) o su dueño (`claim`). El saldo del contrato siempre es lo juntado por las campañas sin liberar más lo pendiente de cobro (hay una prueba que lo comprueba paso a paso). `pay`, `gift`, los pases y las demás funciones siguen sin custodia.

La v0.7.0 suma **fondos de patrocinio**, la segunda custodia: alguien deja dinero en el contrato para pagar lecturas de otros.

- `fund_pool`: un patrocinador (una facultad, una biblioteca) deposita para las lecturas de una editorial y nombra a un operador, con un tope por lectura;
- `pay_sponsored`: el operador paga una lectura con el fondo. Quien lee no paga ni firma, y puede no tener cuenta en la red;
- `withdraw_pool`: el patrocinador retira lo que no se usó, cuando quiera.

El operador solo puede pagarle a la editorial del fondo, hasta el tope, y una vez por artículo y lector cada 24 h. No puede retirar. El dinero de un fondo tiene dos salidas: una lectura de esa editorial o de vuelta a su patrocinador.

La v0.8.0 suma **recompensas por revisar o reproducir** un artículo, la tercera custodia:

- `post_bounty`: alguien deja una recompensa en el contrato, con un plazo;
- `award_bounty`: quien la publicó la reparte entre quienes hicieron el trabajo (98%) y la tesorería (2%);
- `cancel_bounty`: si nadie la ganó, quien la publicó la recupera, pero solo después del plazo.

Quién gana lo decide quien publicó la recompensa. El contrato garantiza que el dinero existe, que no se retira antes del plazo y que sale completo, una sola vez.

La v0.9.0 suma **vales de lectura**, la cuarta custodia: lecturas prepagadas que se reparten como enlaces.

- `buy_vouchers`: alguien (un profesor, un congreso) paga por adelantado varios vales para el catálogo de una editorial. Cada vale es un par de llaves ed25519: la pública va al contrato y la privada, en el enlace;
- `redeem_voucher`: quien tiene el enlace firma qué artículo quiere y para qué lector. Con esa firma, cualquiera envía el canje y el contrato paga la lectura con el vale;
- `reclaim_vouchers`: quien compró recupera los vales que vencieron sin usarse.

La firma de un vale solo sirve para ese lector, ese artículo y este contrato. Quien lee no necesita cuenta en la red ni USDC.

La v0.10.0 suma la **cuenta abierta**, la quinta custodia: leer ahora y pagar todo junto con una firma.

- `open_tab`: quien lee deja una garantía y nombra a un operador (el sitio);
- `charge_tab` (de soporte, no mueve fondos): el operador anota cada lectura como deuda con su editorial. La deuda nunca pasa de la garantía;
- `settle_tab`: quien lee paga toda la deuda con una firma, y cada editorial cobra su 98%;
- `collect_tab`: si no pagó a tiempo, cualquiera cobra la deuda de la garantía;
- `close_tab`: sin deuda, quien lee recupera su garantía.

El operador solo anota deuda: no puede mover la garantía ni cerrar la cuenta. Lo que se debe siempre está cubierto, porque la deuda no pasa de la garantía.

La v0.11.0 suma el **Pase PaperPay**, la sexta custodia: un pase para todo el catálogo, que se reparte entre las editoriales según lo que esa persona leyó.

- `buy_global_pass`: quien lee paga el pase; el dinero queda en el contrato hasta que vence;
- `use_pass` (de soporte, no mueve fondos): el operador anota cada lectura hecha con el pase;
- `settle_pass`: vencido el pase, cualquiera lo liquida: 2% a la tesorería y el resto entre las editoriales, en proporción a las lecturas. Si no leyó nada, vuelve todo;
- `cancel_pass`: mientras no haya leído nada, quien lo compró lo cancela y recupera todo.

A diferencia de `buy_pass`, que es de una sola editorial y le paga al momento, aquí nadie cobra hasta saber qué se leyó.

La v0.12.0 suma el **journal club**, la séptima custodia: un grupo junta una caja común y paga las lecturas que aprueba.

- `create_club` (de soporte): alguien define la cuota y cuántas aprobaciones hacen falta para pagar;
- `join_club`: cada miembro paga su cuota a la caja del club;
- `propose_read` (de soporte): un miembro propone pagar un artículo con la caja;
- `approve_read`: otro miembro aprueba; la aprobación que completa el quórum paga a la editorial y a la tesorería desde la caja;
- `leave_club`: quien sale se lleva una parte igual de lo que queda.

Quien crea el club no tiene ningún poder sobre la caja: el dinero solo sale por una lectura que aprobó el quórum o con cada miembro que se va.

La v0.13.0 suma el **pago contra entrega**, la octava custodia: la editorial solo cobra si entrega la llave que abre el artículo.

- `lock_payment`: quien lee deja el pago en el contrato, atado al sha256 de la llave, con un plazo;
- `unlock_payment`: quien presenta la llave antes del plazo libera el pago a la editorial (98%) y a la tesorería (2%), y la llave queda publicada en el evento;
- `reclaim_payment`: si la llave no llegó a tiempo, quien pagó recupera todo.

El contrato garantiza que la editorial no cobra sin publicar una llave con ese sha256. Que la llave abra de verdad el artículo lo comprueba quien lee contra el archivo cifrado que recibió.

La v0.14.0 suma el **compromiso del revisor**, la novena custodia, sobre las recompensas:

- `accept_bounty`: quien va a hacer el trabajo toma la recompensa y deja una garantía (al menos el 10%);
- `submit_review`: si entrega antes del plazo el hash de su informe, recupera la garantía;
- `forfeit_stake`: si no entregó, desde el plazo su garantía pasa a quien publicó la recompensa.

El contrato comprueba que hubo una entrega a tiempo y deja su hash con fecha. La calidad la juzga quien publicó la recompensa al repartirla.

La v0.15.0 suma la **traducción por encargo**, la décima custodia: un trabajo largo que se paga por hitos.

- `fund_grant`: alguien encarga la traducción de un artículo y deja el pago completo en el contrato, dividido en hitos, con un plazo;
- `release_milestone`: quien paga libera el siguiente hito: 98% a quien traduce y 2% a la tesorería;
- `cancel_grant`: pasado el plazo, quien paga recupera los hitos que no liberó.

Quien traduce sabe que el dinero existe y que no se retira antes del plazo. Si un hito está bien hecho lo decide quien paga.

La v0.16.0 suma el **agente con tope**, sin custodia: un agente de IA paga lecturas con el USDC de su dueño.

- `authorize_agent` (de soporte): el dueño nombra a su agente, con un tope por lectura, un presupuesto total y un vencimiento;
- `pay_as_agent`: el agente paga una lectura solo con su firma; el dinero sale de la wallet del dueño, dentro del tope y del presupuesto, y el acceso queda a nombre del dueño;
- `revoke_agent` (de soporte): el dueño retira el permiso.

El presupuesto es una autorización del token al contrato: el dinero no sale de la wallet del dueño hasta cada pago, y solo puede ir a una editorial y a la tesorería.

La v0.17.0 suma **pagar con el saldo pendiente**:

- `pay_from_owed`: quien tiene dinero pendiente en el contrato (una recompensa, un hito, un cobro que no pudo recibir) paga una lectura con ese saldo, sin retirarlo y sin trustline de USDC.

Es el primer pago que no sale de una wallet: lo paga el contrato con lo que le debe a esa cuenta.

La v0.18.0 suma la **renta con opción a compra**, sin custodia:

- `rent`: quien lee paga la renta de un artículo por un periodo; el contrato guarda cuánto pagó, hasta cuándo y el precio de compra pactado;
- `buy_out`: mientras la renta siga vigente, completa la compra pagando solo la diferencia.

Entre los dos pagos, quien lee paga exactamente el precio de compra, y la editorial y la tesorería reciben lo mismo que con una compra directa.

| Función | Qué hace |
| --- | --- |
| `__constructor(token, treasury, fee_bps, pass_day_price, open_day_price)` | Fija el token (SAC de USDC), la tesorería, la comisión en puntos base (`200` = 2%, máximo `1000`) y, desde la v0.5.0, el precio de un día de pase y de un día de artículo abierto (mayores que 0). No cambia después: no hay admin. |
| `pay(payer, paper_id, publisher, amount) -> Split` | Pide la firma de `payer` sobre la llamada con sus argumentos, mueve `amount − fee` a `publisher` y `fee` a la tesorería, y emite el evento `paid` con `payer` como lector. |
| `pay_batch(payer, items) -> Split` | Varios artículos (`Item { paper_id, publisher, amount }`, hasta 10) con **una sola firma**. Reparte cada uno igual que `pay`, agrupa las transferencias por editorial y emite un `paid` por artículo. Es todo o nada: lista vacía, artículo repetido o inválido, o falta de saldo → no se cobra ninguno. Devuelve la suma. |
| `gift(payer, reader, paper_id, publisher, amount) -> Split` | `payer` paga y el acceso queda a nombre de `reader` (un profesor que paga la lectura de un estudiante, por ejemplo). Mismo reparto que `pay`. Solo firma `payer`. |
| `gift_batch(payer, reader, items) -> Split` | `pay_batch` como regalo: `payer` paga hasta 10 artículos y el acceso de todos queda a nombre de `reader` (la lista de lectura de un curso). Mismas reglas: sin repetidos, todo o nada, un `paid` por artículo con `reader`. Solo firma `payer`. *(v0.3.0)* |
| `tip(payer, paper_id, publisher, amount) -> Split` | Propina a la editorial por un artículo, con el mismo reparto que una lectura. Emite `tipped`, **no** `paid`: no da acceso. Solo firma `payer`. *(v0.3.0)* |
| `refund(publisher, reader, paper_id, amount)` | La editorial le devuelve `amount` a `reader` y queda el evento `refunded`. Solo firma la editorial. El contrato no guarda pagos, así que no comprueba el pago original; la comisión ya cobrada no vuelve, porque la tesorería no firma. *(v0.3.0)* |
| `pay_authors(payer, paper_id, authors, amount) -> Split` | Una lectura con coautores. `authors` es una lista de `Share { account, bps }` (de 1 a 10 autores distintos, cada parte en puntos base de lo que le toca a la editorial; entre todas suman `10000`). La tesorería cobra su comisión igual que en `pay`; cada autor recibe su parte redondeada hacia abajo y el residuo va al **primer autor** (no se pierde ningún stroop). Emite un `paid` con el primer autor como `publisher`. Solo firma `payer`. *(v0.4.0)* |
| `pay_ref(payer, paper_id, publisher, amount, referrer) -> RefSplit` | Una lectura recomendada. La editorial recibe lo mismo que en `pay`; de la comisión, la mitad (`REFERRAL_SHARE_BPS` = 5000, redondeada hacia abajo) va a `referrer` y el resto a la tesorería. Devuelve `RefSplit { publisher, fee, referral }`, donde `fee` es lo que recibe la tesorería. Emite `paid` (con la comisión total en `fee`) y `referred`. `referrer` no puede ser `payer`. Solo firma `payer`. *(v0.4.0)* |
| `buy_pass(payer, reader, publisher, days, amount) -> u64` | Un pase de `days` días (1 a 31) para leer todo el catálogo de `publisher`. Cobra exactamente `days × pass_day_price` (`amount` tiene que ser ese monto, para que quien firma lo vea; si no, `WrongPrice`) y lo reparte 98/2 como una lectura. Los días se suman al pase vigente de `reader`; si ya venció, cuentan desde ahora. Un pase no vence más allá de 90 días desde ahora (`TooLong`). `reader` puede ser otra cuenta (pase de regalo). Solo firma `payer`. Emite `pass` y devuelve el vencimiento (segundos Unix, hora del ledger). *(v0.5.0)* |
| `pass_until(reader, publisher) -> u64` | Hasta cuándo vale el pase; 0 si nunca tuvo. Solo lee. *(v0.5.0)* |
| `subscribe(reader, publisher, days, renewals, amount) -> u64` | Suscripción al catálogo de `publisher`. Cobra ya el primer periodo (como `buy_pass`, para `reader`) y, con la misma firma, autoriza al contrato a cobrar `renewals` renovaciones más (1 a 12) del mismo precio: `approve` del token por `amount × renewals`, con vencimiento un día después de lo que cubre la suscripción completa (máximo 150 días: `TooLong`). El dinero de las renovaciones se queda en la wallet de `reader` hasta cada cobro. Una suscripción por lector: una nueva reemplaza a la anterior y a su autorización. Emite `pass` y `subscribed`. *(v0.5.0)* |
| `renew_subscription(reader) -> u64` | Cobra una renovación con la autorización (`transfer_from`), la reparte 98/2 y suma `days` días al pase. **No pide firmas**: la puede enviar cualquiera (la editorial, PaperPay o quien lee), porque solo cobra el precio pactado y solo cuando al pase le falta un día o menos (`RenewTooEarly`); así nunca se cobra más de un periodo por adelantado. Con la última renovación, la suscripción termina. Emite `pass` y `renewed`. *(v0.5.0)* |
| `cancel_subscription(reader)` | Borra las renovaciones pendientes y deja en 0 la autorización del token. El pase ya pagado sigue hasta su vencimiento. Firma `reader`. Emite `unsubscribed`. *(v0.5.0)* |
| `subscription(reader) -> Option<Subscription>` | La suscripción vigente: `{ publisher, days, amount, renewals_left, allowance_until }`. Solo lee. *(v0.5.0)* |
| `sponsor_open(sponsor, paper_id, publisher, days, amount) -> u64` | Un laboratorio, una fundación o la misma editorial paga para que `paper_id` quede **abierto para todos** `days` días (1 a 31). Cobra exactamente `days × open_day_price` y lo reparte 98/2. Los días se suman a la apertura vigente, con el mismo tope de 90 días. Solo firma `sponsor`. Emite `opened`, no `paid`: no es la lectura de una cuenta. Devuelve hasta cuándo queda abierto. *(v0.5.0)* |
| `open_until(publisher, paper_id) -> u64` | Hasta cuándo está abierto para todos el artículo; 0 si nunca se abrió. Solo lee. *(v0.5.0)* |
| `open_campaign(publisher, paper_id, goal, deadline) -> u32` | La oferta pública de la editorial: liberar `paper_id` para todos si los aportes llegan a `goal` antes de `deadline` (segundos Unix, hora del ledger; como mucho 90 días desde ahora, si no `InvalidDeadline`). Solo firma `publisher` y no mueve fondos. Los ids son secuenciales desde 0, así que nadie ocupa la campaña de otra editorial. Emite `campaign` y devuelve el id. *(v0.6.0)* |
| `pledge(backer, campaign_id, amount) -> i128` | Un aporte: pasa `amount` de la wallet de `backer` al contrato, **en garantía**. Solo mientras la campaña está abierta (antes del plazo, sin la meta cumplida y sin liberar; si no, `CampaignClosed`) y sin pasar de lo que falta (`OverGoal`). El monto firmado es el que se cobra, nunca se recorta. Solo firma `backer`. Emite `pledged` y devuelve lo juntado hasta ahora. *(v0.6.0)* |
| `release_pledges(campaign_id) -> Split` | Con la meta cumplida (si no, `GoalNotReached`), paga lo juntado: 98% a la editorial y 2% a la tesorería, y marca la campaña como liberada (una sola vez: `CampaignClosed`). **No pide firmas**: el dinero solo puede ir a la editorial de la campaña y a la tesorería. Vale antes o después del plazo. Si la editorial no puede recibir, su parte queda pendiente a su nombre (`credited`). Emite `released`. *(v0.6.0)* |
| `refund_pledge(campaign_id, backer) -> i128` | Si la campaña falló (venció sin llegar a la meta; si no, `CampaignNotFailed`), devuelve completo, sin comisión, el aporte de `backer` (si no tiene, `NothingToRefund`). **No pide firmas**: el dinero solo puede volver a `backer`. Si `backer` ya no puede recibir, queda pendiente para `claim`. Emite `unpledged`. *(v0.6.0)* |
| `claim(account) -> i128` | Cobra todo lo pendiente de `account` (si no hay, `NothingToClaim`) y, con la misma firma, le abre la trustline de USDC con `trust` del SAC (CAP-73) si no la tiene. La cuenta necesita ~0.5 XLM libres para la reserva; si no puede recibir, la transacción falla y lo pendiente sigue intacto. Firma `account`. Emite `claimed`. *(v0.6.0)* |
| `campaign(campaign_id) -> Option<Campaign>` · `pledge_of(campaign_id, backer) -> i128` · `owed(account) -> i128` | La campaña (`{ publisher, paper_id, goal, deadline, raised, released }`), lo que aportó una cuenta y lo que tiene pendiente. Solo leen. *(v0.6.0)* |
| `fund_pool(sponsor, publisher, operator, max_per_read, amount) -> i128` | `sponsor` deposita `amount` en su fondo para las lecturas de `publisher`: el USDC pasa al contrato. `operator` es la única cuenta que puede gastarlo, hasta `max_per_read` por lectura. Una recarga se suma al saldo y reemplaza operador y tope. Solo firma `sponsor`. Emite `pool_funded` y devuelve el saldo. **Desde la v0.7.0.** |
| `pay_sponsored(operator, sponsor, reader, paper_id, publisher, amount) -> Split` | El operador paga una lectura con el fondo de `sponsor` para `publisher`: 98/2 directo desde el contrato. Solo firma `operator`. Falla si no es el operador (`NotOperator`), si pasa del tope (`AboveMaxPerRead`) o del saldo (`PoolTooLow`), o si ese fondo ya pagó ese artículo para ese lector en las últimas 24 h (`AlreadySponsored`). Si la editorial no puede recibir, falla y el fondo queda intacto. Emite `paid` (con `payer = sponsor`) y `sponsored`. **Desde la v0.7.0.** |
| `withdraw_pool(sponsor, publisher, amount) -> i128` | `sponsor` retira `amount` de su fondo, del contrato a su wallet. Solo firma `sponsor`. Emite `pool_withdrawn` y devuelve lo que queda. **Desde la v0.7.0.** |
| `pool(sponsor, publisher) -> Option<Pool>` | El fondo (`operator`, `max_per_read`, `balance`), sin mover fondos. **Desde la v0.7.0.** |
| `post_bounty(funder, paper_id, amount, deadline) -> u32` | `funder` deja `amount` en el contrato como recompensa por revisar o reproducir `paper_id`, hasta `deadline` (segundos Unix, a no más de 90 días). Solo firma `funder`. Emite `bounty_posted` y devuelve el id. **Desde la v0.8.0.** |
| `award_bounty(funder, bounty_id, winners) -> Split` | `funder` reparte su recompensa entre `winners` (`Share { account, bps }`, de 1 a 10 cuentas que suman 100%); el 2% va a la tesorería. Quien no puede recibir queda con su parte pendiente para `claim`. `funder` no puede ser ganador. Antes o después del plazo, una sola vez. Emite un `rewarded` por ganador y `bounty_awarded`. **Desde la v0.8.0.** |
| `cancel_bounty(funder, bounty_id) -> i128` | `funder` recupera su recompensa completa, sin comisión. Solo después del plazo (`BountyActive` antes) y si no se repartió. Emite `bounty_cancelled`. **Desde la v0.8.0.** |
| `bounty(bounty_id) -> Option<Bounty>` | La recompensa (`funder`, `paper_id`, `amount`, `deadline`, `open`), sin mover fondos. **Desde la v0.8.0.** |
| `buy_vouchers(buyer, publisher, keys, value, expires_at) -> i128` | `buyer` paga `value` por cada llave pública ed25519 de `keys` (de 1 a 10): cada una es un vale por una lectura del catálogo de `publisher`, vigente hasta `expires_at` (a no más de 90 días). El total pasa al contrato. Una llave no se puede usar dos veces (`VoucherExists`). Solo firma `buyer`. Emite `vouchers_bought` y devuelve el total. **Desde la v0.9.0.** |
| `redeem_voucher(key, signature, reader, paper_id) -> Split` | Canjea el vale de `key`: paga `paper_id` para `reader`, 98/2 desde el contrato a la editorial del vale. **No pide la firma de ninguna cuenta:** `signature` es la firma ed25519 de la llave del vale sobre `(contrato, reader, paper_id)` en XDR, y cualquiera puede enviarla. Una vez, y antes de vencer. Emite `paid` (con `payer` = quien compró) y `voucher_redeemed`. **Desde la v0.9.0.** |
| `reclaim_vouchers(buyer, keys) -> i128` | `buyer` recupera el valor de sus vales vencidos sin usar, sin comisión. Si uno de la lista no es suyo, ya se usó o sigue vigente, no se devuelve ninguno. Solo firma `buyer`. Emite `vouchers_reclaimed`. **Desde la v0.9.0.** |
| `voucher(key) -> Option<Voucher>` | El vale (`buyer`, `publisher`, `value`, `expires_at`, `open`), sin mover fondos. **Desde la v0.9.0.** |
| `open_tab(reader, operator, deposit, period)` | `reader` deja `deposit` en el contrato como garantía y nombra a `operator`, la única cuenta que puede anotarle lecturas. `period` son los segundos para pagar desde la primera lectura sin pagar (hasta 31 días). Una cuenta por lector (`TabExists`). Solo firma `reader`. Emite `tab_opened`. **Desde la v0.10.0.** |
| `charge_tab(operator, reader, publisher, paper_id, amount) -> i128` | De soporte, **no mueve fondos**: el operador anota una lectura como deuda de `reader` con `publisher`. Falla si la deuda pasaría de la garantía o de 10 editoriales (`TabFull`). Solo firma `operator`. Emite `charged` y devuelve la deuda total. **Desde la v0.10.0.** |
| `settle_tab(reader) -> Split` | `reader` paga toda su deuda con una firma: el total sale de su wallet; cada editorial recibe el 98% de lo suyo y la tesorería el 2%. Quien no puede recibir queda con su parte pendiente para `claim`. La garantía no se toca. Emite `tab_settled`. **Desde la v0.10.0.** |
| `collect_tab(reader) -> Split` | Cobra de la garantía la deuda vencida de `reader`, con el mismo reparto. Solo después del plazo de pago (`TabNotDue` antes). **Sin firmas:** la envía cualquiera. Emite `tab_collected`. **Desde la v0.10.0.** |
| `close_tab(reader) -> i128` | `reader` cierra su cuenta y recupera la garantía que queda. Solo sin deuda (`TabOwes`). Solo firma `reader`. Emite `tab_closed`. **Desde la v0.10.0.** |
| `tab(reader) -> Option<Tab>` | La cuenta abierta (`operator`, `deposit`, `period`, `owed`, `due`, `debts`), sin mover fondos. **Desde la v0.10.0.** |
| `buy_global_pass(reader, operator, amount, period) -> u64` | `reader` paga `amount` por leer todo el catálogo durante `period` segundos (hasta 31 días). El dinero pasa al contrato hasta que el pase vence. `operator` es la única cuenta que puede anotarle lecturas y quien decide si el monto alcanza. Un pase a la vez (`PassExists`). Solo firma `reader`. Emite `global_pass` y devuelve cuándo vence. **Desde la v0.11.0.** |
| `use_pass(operator, reader, publisher, paper_id) -> u32` | De soporte, **no mueve fondos**: el operador anota una lectura de `publisher` hecha con el pase de `reader`. Solo mientras está vigente (`PassExpired`) y hasta 10 editoriales (`PassFull`). Solo firma `operator`. Emite `pass_read` y devuelve el total de lecturas. **Desde la v0.11.0.** |
| `settle_pass(reader) -> Split` | Liquida el pase vencido de `reader`: 2% a la tesorería y el resto entre las editoriales en proporción a sus lecturas. Sin lecturas, todo vuelve a `reader`. Solo después de vencer (`PassActive`). **Sin firmas:** la envía cualquiera. Emite un `pass_share` por editorial y `pass_settled`. **Desde la v0.11.0.** |
| `cancel_pass(reader) -> i128` | `reader` cancela su pase y recupera todo, mientras no tenga lecturas (`PassUsed`). Solo firma `reader`. Emite `pass_cancelled`. **Desde la v0.11.0.** |
| `global_pass(reader) -> Option<GlobalPass>` | El pase sin liquidar (`operator`, `amount`, `until`, `reads`, `by_publisher`), sin mover fondos. **Desde la v0.11.0.** |
| `create_club(founder, dues, quorum) -> u32` | De soporte, **no mueve fondos**: crea un journal club con cuota `dues` y un quórum de `quorum` aprobaciones (de 2 a 10). Nadie es miembro hasta que paga su cuota. Solo firma `founder`. Emite `club_created` y devuelve el id. **Desde la v0.12.0.** |
| `join_club(member, club_id) -> i128` | `member` paga la cuota del club: pasa de su wallet a la caja, en el contrato. Hasta 10 miembros (`ClubFull`), una vez cada uno (`AlreadyMember`). Solo firma `member`. Emite `club_joined` y devuelve la caja. **Desde la v0.12.0.** |
| `propose_read(member, club_id, publisher, paper_id, amount) -> u32` | De soporte, **no mueve fondos**: un miembro propone pagar `amount` a `publisher` por `paper_id` con la caja, si alcanza (`ClubTooLow`). Su aprobación queda contada. Solo firma `member`. Emite `read_proposed` y devuelve el id de la propuesta. **Desde la v0.12.0.** |
| `approve_read(member, club_id, proposal_id) -> bool` | Un miembro aprueba una propuesta. Cuando las aprobaciones de miembros actuales llegan al quórum, **esta llamada paga**: 98% a la editorial y 2% a la tesorería, desde la caja. Una aprobación por miembro (`AlreadyApproved`), un pago por propuesta (`ProposalClosed`). Solo firma `member`. Emite `read_approved` y, si pagó, `club_paid`. Devuelve si pagó. **Desde la v0.12.0.** |
| `leave_club(member, club_id) -> i128` | `member` sale y se lleva la caja entre los miembros actuales (redondeada hacia abajo); el último se lleva todo lo que queda. Sus aprobaciones dejan de contar. Solo firma `member`. Emite `club_left`. **Desde la v0.12.0.** |
| `club(club_id) -> Option<Club>` y `proposal(club_id, proposal_id) -> Option<Proposal>` | El club (`dues`, `quorum`, `members`, `balance`, `proposals`) y una propuesta (`publisher`, `paper_id`, `amount`, `approvals`, `paid`), sin mover fondos. **Desde la v0.12.0.** |
| `lock_payment(reader, publisher, paper_id, amount, key_hash, deadline) -> u32` | `reader` deja `amount` en el contrato por `paper_id` de `publisher`, atado a `key_hash` (sha256 de la llave de 32 bytes que abre el artículo), hasta `deadline` (segundos Unix, a no más de 7 días). Solo firma `reader`. Emite `payment_locked` y devuelve el id. **Desde la v0.13.0.** |
| `unlock_payment(lock_id, key) -> Split` | Libera el pago con la llave: si su sha256 es el pactado (`WrongKey` si no) y el plazo no venció (`LockExpired`), paga 98% a la editorial y 2% a la tesorería. **No pide firma**: la envía cualquiera y el dinero solo va a la editorial del pago. Una editorial sin trustline queda con su parte pendiente para `claim`. Emite `paid` y `payment_unlocked` (con la llave). **Desde la v0.13.0.** |
| `reclaim_payment(reader, lock_id) -> i128` | `reader` recupera su pago completo, sin comisión. Solo desde el plazo (`LockActive` antes) y si no se cobró. Solo firma `reader`. Emite `payment_reclaimed`. **Desde la v0.13.0.** |
| `lock(lock_id) -> Option<Lock>` | El pago contra entrega (`reader`, `publisher`, `paper_id`, `amount`, `key_hash`, `deadline`, `open`), sin mover fondos. **Desde la v0.13.0.** |
| `accept_bounty(reviewer, bounty_id, stake) -> u64` | `reviewer` toma la recompensa y deja `stake` en el contrato: al menos el 10% de la recompensa (`StakeTooLow`), una vez por cuenta (`AlreadyAccepted`), solo si la recompensa está abierta y antes de su plazo (`BountyExpired`). Solo firma `reviewer`. Emite `bounty_accepted` y devuelve el plazo de entrega. **Desde la v0.14.0.** |
| `submit_review(reviewer, bounty_id, report_hash) -> i128` | `reviewer` entrega antes del plazo: `report_hash` (sha256 del informe) queda en la red y su garantía vuelve completa. Vale aunque la recompensa ya se haya repartido. Solo firma `reviewer`. Emite `review_submitted`. **Desde la v0.14.0.** |
| `forfeit_stake(bounty_id, reviewer) -> i128` | Desde el plazo (`BountyActive` antes), la garantía de quien no entregó pasa completa, sin comisión, a quien publicó la recompensa. **No pide firma**. Si esa cuenta no puede recibir, queda pendiente para `claim`. Emite `stake_forfeited`. **Desde la v0.14.0.** |
| `stake(bounty_id, reviewer) -> i128` | La garantía de esa cuenta en esa recompensa (0 si no tiene), sin mover fondos. **Desde la v0.14.0.** |
| `fund_grant(funder, translator, paper_id, milestones, deadline) -> u32` | `funder` deja en el contrato la suma de `milestones` (de 1 a 10 montos mayores que cero) para que `translator` traduzca `paper_id`, hasta `deadline` (segundos Unix, a no más de 90 días). Solo firma `funder`. Emite `grant_funded` y devuelve el id. **Desde la v0.15.0.** |
| `release_milestone(funder, grant_id) -> Split` | `funder` libera el siguiente hito, en orden: 98% a quien traduce y 2% a la tesorería. El último cierra el encargo. Antes o después del plazo, mientras no se haya cancelado. Quien no puede recibir queda con su parte pendiente para `claim`. Emite `milestone_released`. **Desde la v0.15.0.** |
| `cancel_grant(funder, grant_id) -> i128` | `funder` recupera los hitos no liberados, sin comisión. Solo desde el plazo (`GrantActive` antes). Emite `grant_cancelled`. **Desde la v0.15.0.** |
| `grant(grant_id) -> Option<Grant>` | El encargo (`funder`, `translator`, `paper_id`, `milestones`, `released`, `deadline`, `open`), sin mover fondos. **Desde la v0.15.0.** |
| `authorize_agent(owner, agent, max_per_read, budget, days) -> u64` | De soporte, **no mueve fondos**: `owner` autoriza a `agent` a pagar lecturas con su USDC, hasta `max_per_read` por lectura y `budget` en total (una allowance del token al contrato), durante `days` días (de 1 a 30). Un permiso nuevo reemplaza al anterior. No se puede con una suscripción activa (`AllowanceInUse`). Solo firma `owner`. Emite `agent_authorized` y devuelve cuándo vence. **Desde la v0.16.0.** |
| `pay_as_agent(agent, owner, publisher, paper_id, amount) -> Split` | `agent` paga una lectura con el USDC de `owner`: 98% a la editorial y 2% a la tesorería, directo desde la wallet de `owner`. **Solo firma `agent`.** Falla si no es el agente de ese dueño (`NoAgent`), si el permiso venció (`AgentExpired`), si pasa del tope (`AboveMaxPerRead`) o de lo que queda del presupuesto (`BudgetExhausted`). El acceso queda a nombre de `owner`. Emite `paid` y `agent_paid`. **Desde la v0.16.0.** |
| `revoke_agent(owner)` | De soporte, **no mueve fondos**: retira el permiso y deja en cero la autorización del token (si hoy es de una suscripción, no la toca). Solo firma `owner`. Emite `agent_revoked`. **Desde la v0.16.0.** |
| `agent(owner) -> Option<AgentPermit>` | El permiso (`agent`, `max_per_read`, `expires_at`), sin mover fondos. Lo que queda del presupuesto es la allowance de `owner` al contrato en el token. **Desde la v0.16.0.** |
| `pay_from_owed(account, publisher, paper_id, amount) -> Split` | `account` paga una lectura con lo que el contrato le debe: 98% a la editorial y 2% a la tesorería, desde la custodia. Si lo pendiente no alcanza, `OwedTooLow`. Lo que sobra sigue pendiente para `claim`. No hace falta trustline de USDC. Solo firma `account`. Emite `paid` y `paid_from_owed`. **Desde la v0.17.0.** |
| `rent(reader, publisher, paper_id, amount, full_price, period) -> u64` | `reader` renta el artículo por `period` segundos (de 1 a 7 días): paga `amount` con el reparto 98/2, directo desde su wallet. `full_price` es el precio de compra y tiene que ser mayor (`RentNotCheaper`). Una renta vigente no se repite (`RentalActive`). Solo firma `reader`. Emite `rented` y devuelve hasta cuándo vale. **Desde la v0.18.0.** |
| `buy_out(reader, publisher, paper_id) -> Split` | `reader` convierte su renta vigente en compra: paga `full_price` menos lo que ya pagó, con el mismo reparto. Falla sin renta (`NoRental`) o con la renta vencida (`RentalExpired`). Solo firma `reader`. Emite `paid` y `bought_out`. **Desde la v0.18.0.** |
| `rental(reader, publisher, paper_id) -> Option<Rental>` | La renta (`paid`, `full_price`, `until`), vigente o vencida, sin mover fondos. **Desde la v0.18.0.** |
| `split(amount) -> Split` | El reparto que aplicaría `pay`, sin mover fondos. |
| `config() -> Config` | La configuración fijada al desplegar. |

- **Comisión:** `fee = amount × fee_bps / 10 000`, redondeada hacia abajo. La fracción de stroop queda para la editorial: de $0.50 (`5 000 000`) salen `4 900 000` para la editorial y `100 000` para la tesorería.
- **Evento `paid`:**
  - topics: `["paid", publisher, reader]`;
  - datos: `{ paper_id, payer, amount, fee }`.
  
  Es el comprobante en la red de qué artículo se pagó y quién lo puede leer. En `pay_batch` y `gift_batch` la comisión se redondea por artículo, así que un paquete cobra lo mismo que los `pay` separados.
- **Evento `tipped`** (v0.3.0): topics `["tipped", publisher, payer]`; datos `{ paper_id, amount, fee }`.
- **Evento `refunded`** (v0.3.0): topics `["refunded", publisher, reader]`; datos `{ paper_id, amount }`.
- **Evento `referred`** (v0.4.0): topics `["referred", referrer, payer]`; datos `{ paper_id, publisher, amount, reward }`. Acompaña al `paid` de la misma lectura: `amount` es el precio y `reward` lo que recibió `referrer`.
- **Evento `pass`** (v0.5.0): topics `["pass", publisher, reader]`; datos `{ payer, days, amount, fee, valid_until }`. Lo emiten `buy_pass`, `subscribe` y `renew_subscription`: `reader` puede leer el catálogo de `publisher` hasta `valid_until`.
- **Eventos de la suscripción** (v0.5.0):
  - `subscribed`: topics `["subscribed", publisher, reader]`; datos `{ days, amount, renewals, allowance_until }`;
  - `renewed`: topics `["renewed", publisher, reader]`; datos `{ days, amount, fee, valid_until, renewals_left }`;
  - `unsubscribed`: topics `["unsubscribed", publisher, reader]`; datos `{ renewals_left }`.
- **Evento `opened`** (v0.5.0): topics `["opened", publisher, sponsor]`; datos `{ paper_id, days, amount, fee, open_until }`.
- **Eventos de las campañas** (v0.6.0):
  - `campaign`: topics `["campaign", publisher]`; datos `{ campaign_id, paper_id, goal, deadline }`;
  - `pledged`: topics `["pledged", backer]`; datos `{ campaign_id, amount, raised }`;
  - `released`: topics `["released", publisher]`; datos `{ campaign_id, paper_id, raised, fee }`. Es distinto de `opened` (`sponsor_open`), que abre un artículo por días;
  - `unpledged`: topics `["unpledged", backer]`; datos `{ campaign_id, amount }`;
  - `credited`: topics `["credited", account]`; datos `{ amount }`: un pago que la cuenta no pudo recibir y quedó pendiente;
  - `claimed`: topics `["claimed", account]`; datos `{ amount }`.
- **Qué guarda la v0.5.0:** en almacenamiento `persistent`, el vencimiento de cada pase (`reader`, `publisher`) y de cada artículo abierto (`publisher`, `paper_id`), y la suscripción de cada lector. Cada entrada se extiende hasta su vencimiento y una semana más. Por eso los topes: 31 días por compra, 90 de acceso acumulado y 150 por suscripción caben en el TTL máximo de testnet (~180 días).
- **Qué guarda la v0.6.0:** cada campaña y cada aporte (`persistent`, hasta el plazo y 30 días más para poder devolver, más la semana de margen: como mucho ~127 días, dentro del TTL máximo) y lo pendiente de cada cuenta (TTL máximo de la red en cada abono). Una entrada que vence se archiva y se puede restaurar con su saldo intacto.
- **La firma de `pledge` no depende del estado de la campaña:** el monto del `transfer` que firma quien aporta es el que pidió, nunca "lo que falte". Si otro aporte entra entre la simulación y el envío, el aporte falla limpio (`OverGoal` o `CampaignClosed`) sin mover nada, en vez de fallar por una firma que ya no coincide.
- **La firma de `subscribe` no depende del ledger exacto:** el vencimiento de la autorización es un argumento de `approve` que quien lee firma. Si dependiera del ledger del momento, la red avanzaría entre la simulación y el envío y rechazaría la firma (pasó en la red local; está en [`docs/QA.md`](../docs/QA.md)). Por eso se redondea al día de ledgers (17 280). Solo cambia si entre simular y enviar la red cruza ese múltiplo, una vez al día; entonces basta reintentar.
- **`pay_authors` en la red:** el `paid` lleva solo al primer autor; lo que recibió cada coautor se ve en las transferencias del token de esa transacción.
- **Acceso:** la API solo da acceso por eventos `paid` (`paidReceipts` en `apps/api/src/services/splitter.service.ts`). Una propina o una devolución nunca abren un artículo. Lo que llegó desde la v0.5.0 (pases, artículos abiertos, campañas, fondos y lo que sigue) existe en testnet, pero la API todavía no lo usa.
- **Quién valida qué:** el contrato garantiza el reparto. La API sigue decidiendo el precio y la editorial de cada artículo, y valida que el evento coincida antes de dar acceso.
- **Errores:** `FeeTooHigh` (1), `InvalidAmount` (2), `InvalidPaperId` (3; vacío o de más de 64 bytes), `AmountOverflow` (4), `EmptyBatch` (5), `BatchTooLarge` (6), `DuplicatePaper` (7), `InvalidShares` (8; partes vacías, en 0, de más del 100%, que no suman 100% o con un autor repetido), `TooManyAuthors` (9; más de 10 coautores), `SelfReferral` (10; `referrer` igual a `payer`), `InvalidDays` (11; 0 o más de 31 días), `WrongPrice` (12; `amount` distinto de días × precio por día), `TooLong` (13; un acceso que vencería a más de 90 días o una suscripción de más de 150), `InvalidRenewals` (14; 0 o más de 12), `NoSubscription` (15), `RenewTooEarly` (16; al pase le falta más de un día), `InvalidDeadline` (17; plazo pasado o a más de 90 días), `NoCampaign` (18), `CampaignClosed` (19; la campaña ya no recibe aportes o ya se liberó), `OverGoal` (20; el aporte pasa de lo que falta), `GoalNotReached` (21), `NothingToClaim` (22), `CampaignNotFailed` (23; todavía no vence, llegó a la meta o se liberó), `NothingToRefund` (24), `NoPool` (25), `NotOperator` (26; quien firma no es el operador del fondo), `AboveMaxPerRead` (27), `PoolTooLow` (28; al fondo no le alcanza para la lectura o el retiro), `AlreadySponsored` (29; el fondo ya pagó ese artículo para ese lector en las últimas 24 h), `NoBounty` (30; no hay recompensa con ese id de esa cuenta), `BountyClosed` (31; ya se pagó o se canceló), `BountyActive` (32; no se cancela antes del plazo), `NoVoucher` (33; no hay vale con esa llave, o no es de esa cuenta), `VoucherSpent` (34; ya se canjeó o se devolvió), `VoucherExpired` (35), `VoucherActive` (36; no se devuelve antes de vencer), `VoucherExists` (37; esa llave ya se usó para un vale), `NoTab` (38), `TabExists` (39; una cuenta abierta por lector), `TabFull` (40; la deuda pasaría de la garantía o de 10 editoriales), `NothingOwed` (41), `TabNotDue` (42; todavía no vence el plazo de pago), `TabOwes` (43; no se cierra con deuda), `NoGlobalPass` (44), `PassExists` (45; un Pase PaperPay a la vez), `PassExpired` (46; ya no se le anotan lecturas), `PassFull` (47; lecturas de más de 10 editoriales), `PassActive` (48; no se liquida antes de vencer), `PassUsed` (49; con lecturas ya no se cancela), `NoClub` (50), `InvalidQuorum` (51; menos de 2 o más de 10), `ClubFull` (52; 10 miembros), `AlreadyMember` (53), `NotMember` (54), `ClubTooLow` (55; a la caja no le alcanza), `NoProposal` (56), `ProposalClosed` (57; ya se pagó), `AlreadyApproved` (58), `NoLock` (59; no hay pago contra entrega con ese id de esa cuenta), `LockClosed` (60; ya se cobró o se devolvió), `WrongKey` (61; el sha256 de la llave no es el pactado), `LockExpired` (62; la llave llegó después del plazo), `LockActive` (63; no se recupera antes del plazo), `BountyExpired` (64; el plazo de la recompensa ya venció), `StakeTooLow` (65; menos del 10% de la recompensa), `AlreadyAccepted` (66; esa cuenta ya la tomó), `NoStake` (67; no la tomó, ya entregó o ya perdió la garantía), `InvalidMilestones` (68; de 1 a 10 hitos, cada uno mayor que cero), `NoGrant` (69; no hay encargo con ese id de esa cuenta), `GrantClosed` (70; ya se pagó completo o se canceló), `GrantActive` (71; no se cancela antes del plazo), `NoAgent` (72; esa cuenta no es el agente de ese dueño), `AgentExpired` (73; el permiso venció), `BudgetExhausted` (74; al presupuesto no le alcanza), `AllowanceInUse` (75; la autorización del token ya es de una suscripción), `OwedTooLow` (76; a lo pendiente de esa cuenta no le alcanza), `NoRental` (77; esa cuenta no tiene una renta de ese artículo), `RentalExpired` (78; la renta venció), `RentalActive` (79; ya hay una renta vigente) y `RentNotCheaper` (80; la renta tiene que costar menos que la compra).

### Probar

Requiere Rust estable y el target `wasm32v1-none` (`rustup target add wasm32v1-none`).

```bash
pnpm test:contracts            # cargo test: 267 pruebas con un SAC de USDC simulado
```

### Compilar el WASM

`soroban-sdk` 29 solo compila el WASM a través de `stellar-cli` v25.2.0 o posterior (no con `cargo build`):

```bash
cd contracts && stellar contract build
```

### Usarlo desde la terminal

Con `stellar-cli` (v30) se puede desplegar un contrato propio y llamar cada función. Son los mismos comandos con los que se probaron la v0.3.0, la v0.4.0, la v0.5.0 y la v0.6.0 en testnet.

**1. Cuentas de prueba con USDC.** Quien cobra (editorial, coautor o referido) necesita la trustline de USDC. Quien paga necesita, además, USDC: en testnet se compra con XLM en el DEX.

```bash
USDC=USDC:GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5
stellar keys generate lector --network testnet --fund      # Friendbot le da XLM
LECTOR=$(stellar keys address lector)
stellar tx new change-trust --line "$USDC" --source lector --network testnet
stellar tx new path-payment-strict-receive --send-asset native --send-max 5000000000 \
  --destination "$LECTOR" --dest-asset "$USDC" --dest-amount 20000000 --source lector --network testnet   # 2 USDC
```

**2. Desplegar** (opcional: también se pueden usar los contratos de la tabla de abajo).

```bash
cd contracts && stellar contract build
WASM=target/wasm32v1-none/release/paperpay_splitter.wasm
stellar contract upload --wasm "$WASM" --source lector --network testnet
stellar contract deploy --wasm-hash "$(sha256sum "$WASM" | cut -c1-64)" --source lector --network testnet \
  -- --token CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA \
     --treasury GAB2NXBPZMJEPZPXMAAEZDJA7SB6ZQRHTT2KF4Z7LMOEAJKSP4JBCWRV --fee_bps 200 \
     --pass_day_price 10000000 --open_day_price 50000000   # desde la v0.5.0: 1 USDC por día de pase, 5 por día abierto
```

**3. Llamar cada función.** Los montos van en stroops (7 decimales: `5000000` = 0.50 USDC). Dentro de un JSON, los `i128` van como texto (`"5000000"`) y los `u32` como número. `--source` es la cuenta que firma: quien paga en todas, salvo en `refund`, donde firma la editorial.

```bash
ID=CC5SIM7JV6TXMWABU7G3DAA3V7HROFBOBF7I5TW4JZHKAQPBFJUWADGA   # v0.4.0, o el contrato que desplegaste
inv() { stellar contract invoke --id "$ID" --network testnet "$@"; }
P=crispr-nanomedicine-biotech
ED=GA3MOPOJ374G4APKR4RE7VPXF5CEQKER3FA3LZ2XSAARIRSZEF6QXUZI      # editorial demo (tiene trustline)

inv --source lector -- pay        --payer "$LECTOR" --paper_id "$P" --publisher "$ED" --amount 5000000
inv --source lector -- pay_batch  --payer "$LECTOR" --items "[{\"paper_id\":\"$P\",\"publisher\":\"$ED\",\"amount\":\"5000000\"}]"
inv --source lector -- gift       --payer "$LECTOR" --reader "$OTRA" --paper_id "$P" --publisher "$ED" --amount 5000000
inv --source lector -- gift_batch --payer "$LECTOR" --reader "$OTRA" --items "[{\"paper_id\":\"$P\",\"publisher\":\"$ED\",\"amount\":\"5000000\"}]"
inv --source lector -- tip        --payer "$LECTOR" --paper_id "$P" --publisher "$ED" --amount 5000000
inv --source lector -- pay_authors --payer "$LECTOR" --paper_id "$P" --amount 5000000 \
  --authors "[{\"account\":\"$ED\",\"bps\":7000},{\"account\":\"$COAUTOR\",\"bps\":3000}]"
inv --source lector -- pay_ref    --payer "$LECTOR" --paper_id "$P" --publisher "$ED" --amount 5000000 --referrer "$REFERIDO"
inv --source editorial -- refund  --publisher "$EDITORIAL" --reader "$LECTOR" --paper_id "$P" --amount 4900000
inv --source lector --send no -- config                       # solo lee; no envía nada

# v0.5.0: accesos con vencimiento (precios de la configuración: 1 USDC por día de pase, 5 por día abierto)
inv --source lector -- buy_pass  --payer "$LECTOR" --reader "$LECTOR" --publisher "$ED" --days 1 --amount 10000000
inv --source lector -- subscribe --reader "$LECTOR" --publisher "$ED" --days 1 --renewals 2 --amount 10000000
inv --source otra   -- renew_subscription --reader "$LECTOR"   # cualquier cuenta la envía; el lector no firma
inv --source lector -- cancel_subscription --reader "$LECTOR"
inv --source lector -- sponsor_open --sponsor "$LECTOR" --paper_id "$P" --publisher "$ED" --days 1 --amount 50000000
inv --source lector --send no -- pass_until --reader "$LECTOR" --publisher "$ED"
inv --source lector --send no -- open_until --publisher "$ED" --paper_id "$P"

# v0.6.0: campaña de acceso abierto (la editorial firma la oferta; meta 1 USDC, plazo en 1 h)
inv --source editorial -- open_campaign --publisher "$EDITORIAL" --paper_id "$P" --goal 10000000 --deadline "$(($(date +%s) + 3600))"
inv --source lector -- pledge --backer "$LECTOR" --campaign_id 0 --amount 10000000   # queda en garantía
inv --source otra   -- release_pledges --campaign_id 0              # meta cumplida: cualquiera la envía
inv --source otra   -- refund_pledge --campaign_id 1 --backer "$LECTOR"   # otra campaña, vencida sin llegar a la meta
inv --source editorial -- claim --account "$EDITORIAL"              # lo pendiente, y abre la trustline
inv --source lector --send no -- campaign --campaign_id 0
```

`$OTRA` es cualquier cuenta: recibir un regalo no pide firma ni trustline, y la cuenta ni siquiera tiene que existir en la red. `$COAUTOR` y `$REFERIDO` sí necesitan la trustline de USDC (paso 1), porque reciben dinero. `refund` necesita una editorial con llave propia (`$EDITORIAL`, cuenta `editorial`) y saldo en USDC. `renew_subscription` la puede enviar cualquier cuenta con XLM para la comisión de red (`otra`), sin USDC.

### Despliegue en testnet

Datos completos en [`deployments/testnet.json`](deployments/testnet.json).

> **Qué versión usa el sitio.** El sitio y la API usan la **v0.4.0** (`productionContractId`) desde el 25 de noviembre de 2026 (CDMX; 00:18 UTC del 26), con `pay`, `pay_ref` (referidos) y `gift`. La v0.4.0 tiene todo lo de la v0.3.0 (`gift_batch`, `tip` y `refund`) más `pay_authors` y `pay_ref`. La v0.2.0, que usó el sitio del 23 al 25 de noviembre, quedó en `SPLITTER_LEGACY_CONTRACT_IDS`: sus regalos se pueden reclamar y sus lecturas siguen en el contador. Los pagos nuevos van solo a la v0.4.0. La **v0.5.0** (pases, suscripción y artículos abiertos) está desplegada y probada en testnet desde el 25 de noviembre de 2026 (CDMX; 05:04 UTC del 26), pero el sitio y la API todavía no la usan. La **v0.6.0** (campañas de acceso abierto y `claim`) está desplegada y probada en testnet desde el 26 de noviembre de 2026 (00:18 CDMX); el sitio y la API tampoco la usan.

| v0.18.0 | Valor |
| --- | --- |
| Contrato | [`CBAXQMKVDTEXVVOOB7KJQJTJ2QNDQ4P3XZJYVXQB6TH3PK4DMXTISCCJ`](https://stellar.expert/explorer/testnet/contract/CBAXQMKVDTEXVVOOB7KJQJTJ2QNDQ4P3XZJYVXQB6TH3PK4DMXTISCCJ) |
| WASM (sha256) | `3e79adb4d459dda30071913b49814226e5a44c8a560e3970adfa381fd78e4b41` (117,169 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.5.0 |
| `rent` | [`6ec274be…`](https://stellar.expert/explorer/testnet/tx/6ec274be792c8a4e3dfda97a778ba7f1f2df17e550439f0ed3c58d549a52c725): 0.10 USDC por 48 h de `crispr-nanomedicine-biotech`, con precio de compra de 0.50. 0.098 a la editorial demo y 0.002 a la tesorería |
| `buy_out` | [`2120bcd3…`](https://stellar.expert/explorer/testnet/tx/2120bcd3918d30e50ca2597218de9be0cca936d45276ace87535d7f6ccce534b): la compra cobra solo los 0.40 que faltan. Entre los dos pagos: 0.49 a la editorial y 0.01 a la tesorería, lo mismo que una compra directa |
| Renta vencida | [`68d8b325…`](https://stellar.expert/explorer/testnet/tx/68d8b3254899a2688c175f57461b605192a9cbe4e10e6dc6d9b847dc6008175e): otra renta, de 45 s. Vencida, la compra con descuento ya no aplica (#78) |

Antes de enviar, el script simuló lo que no debe pasar: comprar sin haber rentado (#77), una renta que no es más barata que la compra (#80), rentar otra vez con la renta vigente (#79), otra cuenta comprando con esa renta (#77), comprar dos veces (#77) y comprar con la renta vencida (#78). El contrato nunca tuvo saldo.

| v0.17.0 | Valor |
| --- | --- |
| Contrato | [`CALLI44DK4TUF3GCPU55V55A6CLBJS2ADZYU4JTPGJCUVN4QZCJZKX2P`](https://stellar.expert/explorer/testnet/contract/CALLI44DK4TUF3GCPU55V55A6CLBJS2ADZYU4JTPGJCUVN4QZCJZKX2P) |
| WASM (sha256) | `a91c26e27307935c4942398f18d06cfc8bdc84e5b8d7bf54d824b1751979a36a` (112,061 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.5.0 |
| Preparación (ya existían) | [`69411321…`](https://stellar.expert/explorer/testnet/tx/69411321b5eef18d13644198584ea0bc15c399ede96ff29503ae1152e5628af4) y [`d7f3dca1…`](https://stellar.expert/explorer/testnet/tx/d7f3dca1abb916cb4244581e4142750e997cbec4778a403e9eae19e2a734c974): una recompensa de 1.00 USDC que gana una revisora **sin trustline**. Sus 0.98 quedan pendientes en el contrato |
| `pay_from_owed` | [`eb6b7150…`](https://stellar.expert/explorer/testnet/tx/eb6b71507e6732bb664d1f1b7fcbc9ac67fffb20acab6b84f78dd1384db5eba4): la revisora paga 0.50 por `quantum-cryptography-post-quantum` con ese saldo. 0.49 a la editorial demo y 0.01 a la tesorería; le quedan 0.48 pendientes |
| Cierre (ya existía) | [`a8d2f39f…`](https://stellar.expert/explorer/testnet/tx/a8d2f39f38a91a5d11accb5c672dc8fdec90b23bb152e67bdfd9589046f6db6e): cobra los 0.48 con `claim`, que le abre la trustline. El contrato queda en 0 |

Antes de enviar, el script simuló lo que no debe pasar: pagar más de lo pendiente (#76), una cuenta a la que no se le debe nada (#76) y pagar 0.50 con 0.48 pendientes (#76).

| v0.16.0 | Valor |
| --- | --- |
| Contrato | [`CCMUFZQR543HAJYYWMVIMA2A47VL2YVEJBRQMJA2F3UELAXAIPWEWBYZ`](https://stellar.expert/explorer/testnet/contract/CCMUFZQR543HAJYYWMVIMA2A47VL2YVEJBRQMJA2F3UELAXAIPWEWBYZ) |
| WASM (sha256) | `10b35983349911698bba5b99776605003d66111578dd2a2fceb001154ffc474a` (110,340 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.5.0 |
| `authorize_agent` (soporte) | [`ab4ee2e0…`](https://stellar.expert/explorer/testnet/tx/ab4ee2e08050219d68410b0bc5fd27545d90606e65da60ab00c316e46c269796): la dueña autoriza a su agente: hasta 0.50 USDC por lectura, 1.20 en total, 1 día. No se mueve nada |
| `pay_as_agent` | [`8a1b1576…`](https://stellar.expert/explorer/testnet/tx/8a1b1576b0806ae35cf0e23cfb2ddbce2025d27f18b967a304506a286f942658): el agente, que no tiene USDC ni trustline, paga `crispr-nanomedicine-biotech` solo con su firma. 0.49 a la editorial demo y 0.01 a la tesorería, desde la wallet de la dueña. Después paga otro artículo ([`38c9dafd…`](https://stellar.expert/explorer/testnet/tx/38c9dafd994ee031e0ab051df80519e13b1b449510d560a38949e09b709d84eb)) y al presupuesto le quedan 0.20 |
| `revoke_agent` (soporte) | [`03a47c0f…`](https://stellar.expert/explorer/testnet/tx/03a47c0f0ab975b06353f53dc9c81079457a7c60ea33ebd362760eccedceadc6): la dueña retira el permiso; la autorización del token queda en 0 |

Antes de enviar, el script simuló lo que no debe pasar: pagar sin permiso (#72), pasar del tope por lectura (#27), otra cuenta pagando como si fuera el agente (#72), pagar más de lo que queda del presupuesto (#74) y pagar con el permiso retirado (#72). El contrato nunca tuvo saldo.

| v0.15.0 | Valor |
| --- | --- |
| Contrato | [`CBFYNOBCVMLQF3DACYYU24FLWDHJLR4ZSZZT2OJ4RKXEFYBZKF5EHXAV`](https://stellar.expert/explorer/testnet/contract/CBFYNOBCVMLQF3DACYYU24FLWDHJLR4ZSZZT2OJ4RKXEFYBZKF5EHXAV) |
| WASM (sha256) | `9a5e9bdb3ca567900d16b45bd0a1df1dbb18917ad56a36bf8baf415d38641800` (104,267 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.5.0 |
| `fund_grant` | [`f5e843b4…`](https://stellar.expert/explorer/testnet/tx/f5e843b4ba026f570b7794dc502dd35e439227eea3a9c901738def0a932ad1d3): 2.00 USDC para traducir `crispr-nanomedicine-biotech`, en tres hitos (0.50, 0.50 y 1.00), con plazo de 1 h (encargo 0) |
| `release_milestone` | [`00f59471…`](https://stellar.expert/explorer/testnet/tx/00f594717bca572baace5dd756a564dbd6c642548cf4600954a7501e92025dde): primer hito. La traductora **no tiene trustline**: sus 0.49 quedan pendientes y los cobra con [`claim` `2c5c01c0…`](https://stellar.expert/explorer/testnet/tx/2c5c01c00e0432c194e25e0701eef3071007ed2b87484ce8d56ff74eadf19d60). Después, los otros dos ([`ecc18e47…`](https://stellar.expert/explorer/testnet/tx/ecc18e472cd41c61bc3ba48dd05404a0385a0e43d957381922c2589c4c3d56c7) y [`293355f0…`](https://stellar.expert/explorer/testnet/tx/293355f0f09198d7bf5c3a8bdf0315c4096800554a690c14af2fd3f7eda2f1e1)): en total 1.96 para ella y 0.04 a la tesorería |
| `cancel_grant` | Encargo 1 de 0.30 en dos hitos, con plazo de 90 s ([`519cd1ed…`](https://stellar.expert/explorer/testnet/tx/519cd1edce93d0701cf2a5cf0b445e58458498096df0d2cddc51cf60b47df20f)). Se libera el primero ([`b7ddcc89…`](https://stellar.expert/explorer/testnet/tx/b7ddcc89ba8f758138e9854478d2fe7842809420bcc074cab1fc940f5c70148c)) y, vencido el plazo, [`ce133f19…`](https://stellar.expert/explorer/testnet/tx/ce133f19589d788efcaa895dff5d4dd3d3b980a0f63366c59212edafdeda5133): vuelven los 0.20 del hito no liberado |

Antes de enviar, el script simuló lo que no debe pasar: quien traduce liberándose un hito (#69), cancelar antes del plazo (#71), liberar un encargo ya pagado (#70) y cancelar dos veces (#70). Al final, el contrato tiene saldo 0.

| v0.14.0 | Valor |
| --- | --- |
| Contrato | [`CBGHC7PRFEOKYP6NQQB4ZIXLFHBZUDB7OJIBOJ6L73HFCZ2IR6YSESYP`](https://stellar.expert/explorer/testnet/contract/CBGHC7PRFEOKYP6NQQB4ZIXLFHBZUDB7OJIBOJ6L73HFCZ2IR6YSESYP) |
| WASM (sha256) | `349fbbdc3e44b32bb91db25946be1375d8643d9871feb1adb52624e9cf576910` (97,785 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.5.0 |
| `post_bounty` (ya existía) | [`b6e368a6…`](https://stellar.expert/explorer/testnet/tx/b6e368a6dec7b250d7e10e27827c831d1a028f5ba6dfc23131bbdb025ae85c14): 1.00 USDC por reproducir `crispr-nanomedicine-biotech`, con plazo de 180 s |
| `accept_bounty` | [`f3de4f9b…`](https://stellar.expert/explorer/testnet/tx/f3de4f9bd107dcdce313afcd7404f489c8c1a1bbb03300a3db60bd62e11b5242) y [`36cac926…`](https://stellar.expert/explorer/testnet/tx/36cac926aaa8ac84d49966211e56fc2b900ab4e48da3e54902f811b5472067eb): Ana y Beto toman la recompensa con 0.10 de garantía cada uno. El contrato guarda 1.20 |
| `submit_review` | [`b2ab9434…`](https://stellar.expert/explorer/testnet/tx/b2ab943481cd813417996fe679616b8ca5186b558aa7b5393dc33839465b088f): Ana entrega a tiempo el hash de su informe (`85be1b73…`) y recupera sus 0.10 |
| `forfeit_stake` | [`fb5256d1…`](https://stellar.expert/explorer/testnet/tx/fb5256d12cee842d4728eaf6aa48800a32e4d13da97653f2b8d42aa294f985ae): Beto no entregó. Pasado el plazo, una cuenta cualquiera ejecuta su garantía: 0.10 a quien publicó la recompensa |
| `award_bounty` (ya existía) | [`88d3f1bb…`](https://stellar.expert/explorer/testnet/tx/88d3f1bb5155cc4b3b16503eeb7bdf65341d09d6039d122c1500fb084bcdde69): la recompensa va a Ana, que sí entregó: 0.98 para ella y 0.02 a la tesorería |

Antes de enviar, el script simuló lo que no debe pasar: una garantía menor al 10% (#65), tomarla dos veces (#66), ejecutar la garantía antes del plazo (#32), entregar sin haber dejado garantía (#67), entregar después del plazo (#64) y ejecutar la garantía dos veces (#67). Al final, el contrato tiene saldo 0.

| v0.13.0 | Valor |
| --- | --- |
| Contrato | [`CDHBL4S7H5QEXL7HEP47NUOGJ4JBJDVG6WHBG6GNTUMG7TIF3LT73HSJ`](https://stellar.expert/explorer/testnet/contract/CDHBL4S7H5QEXL7HEP47NUOGJ4JBJDVG6WHBG6GNTUMG7TIF3LT73HSJ) |
| WASM (sha256) | `359985bc647adb3c3bea22c29604c8d8f833bd15ffcf76244dc80ec711cb9c6c` (92,911 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.5.0 |
| `lock_payment` | [`85f946b1…`](https://stellar.expert/explorer/testnet/tx/85f946b15692159ba7d1830a1a722a08e5e260c9f09737be5081e1783853c24d): 0.50 USDC por `crispr-nanomedicine-biotech`, bloqueados contra el hash `39aa557a…` con plazo de 1 h (pago 0) |
| `unlock_payment` | [`670863f5…`](https://stellar.expert/explorer/testnet/tx/670863f5817621ee10a7ebae8a036995f18b01016ebf4a6ca9870fdd1a5556e5): una cuenta que no es la editorial ni quien pagó presenta la llave `d75ba187…`. 0.49 a la editorial demo y 0.01 a la tesorería; la llave queda en el evento `payment_unlocked` |
| `reclaim_payment` | Pago 1 de 0.30 con plazo de 90 s ([`0bbf88f7…`](https://stellar.expert/explorer/testnet/tx/0bbf88f74bd27ac2a30331277cfb33257bcd1c6941a364b08f30bae5f760ef3c)). La llave no llega: [`a8943076…`](https://stellar.expert/explorer/testnet/tx/a8943076a34bc1e0a1e0a7c921ca9e957bf1b6db5a9ec8f55ebadb08aa14617b), vuelve completo a quien pagó |

Antes de enviar, el script simuló lo que no debe pasar: liberar con otra llave (#61), presentar el hash como si fuera la llave (#61), recuperar antes del plazo (#63), otra cuenta recuperando (#59), liberar dos veces (#60), la llave llegando después del plazo (#62) y recuperar dos veces (#60). Al final, el contrato tiene saldo 0.

| v0.12.0 | Valor |
| --- | --- |
| Contrato | [`CCFNCZVQFRYDFRUE6CD25QVT7RRRFKMFHAU6TB7U6HWFPECLUQECHIY6`](https://stellar.expert/explorer/testnet/contract/CCFNCZVQFRYDFRUE6CD25QVT7RRRFKMFHAU6TB7U6HWFPECLUQECHIY6) |
| WASM (sha256) | `87930abdbd1ec366a55e72dd97ce3eecfcb38b20483f44f444a63f71d3b9f4b6` (86,618 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.5.0 |
| `create_club` (soporte) | [`b12604ec…`](https://stellar.expert/explorer/testnet/tx/b12604ec7e63f3982b17fa3fc625098a7db84b33f34ce051fa026c6ae1da93f5): un club con cuota de 0.50 USDC y quórum de 2. No se mueve nada |
| `join_club` | [`55f701c0…`](https://stellar.expert/explorer/testnet/tx/55f701c0dbd2fc38fb27af99d8a85b44373ec499a53cd9042fd3f5bb51291c6e), [`6861f99e…`](https://stellar.expert/explorer/testnet/tx/6861f99e9e50a43dffa57426619395ad1d723dcc02226c3b64aaf58e137d08ca) y [`dc94e6a4…`](https://stellar.expert/explorer/testnet/tx/dc94e6a4cea73211c42798fb17a911e00444f93ce743e2ad1cc8a1d1ff59421f): Ana, Beto y Caro pagan su cuota. La caja queda en 1.50, en el contrato |
| `propose_read` (soporte) | [`93e67d0f…`](https://stellar.expert/explorer/testnet/tx/93e67d0fc0b32ef48f071e52eda329eba545f312c0fa67a14a83ff778f6767e7): Ana propone pagar `crispr-nanomedicine-biotech` por 0.50. No se mueve nada |
| `approve_read` | [`987b89f2…`](https://stellar.expert/explorer/testnet/tx/987b89f299eeb27a39e262745b3f487f122aa537936c8be3d0470ee91b1f4633): Beto aprueba, se completa el quórum y **esa misma llamada paga**: 0.49 a la editorial demo y 0.01 a la tesorería, desde la caja. Evento `club_paid` con los tres miembros |
| `leave_club` | [`c25c6345…`](https://stellar.expert/explorer/testnet/tx/c25c6345ef643aed6e346a46ee0c4ed133d37b358b5d52a43e122866cdf4c1c6): Caro sale con un tercio de lo que queda (0.3333333). Después salen Beto ([`ff865cc8…`](https://stellar.expert/explorer/testnet/tx/ff865cc8af0a78d5c704f43167ac099b199084380c39449385af2bb3ecee6c63)) y Ana ([`dff63031…`](https://stellar.expert/explorer/testnet/tx/dff630317bfab578f6844a54bce4021f2fc83eb856463cf04c0d95eac60811df)), que se lleva el stroop del redondeo |

Antes de enviar, el script simuló lo que no debe pasar: un quórum de 1 (#51), entrar dos veces (#53), un extraño aprobando (#54), aprobar dos veces (#58), proponer más que la caja (#55), aprobar una propuesta ya pagada (#57) y proponer después de salir (#54). Al final, el contrato tiene saldo 0.

| v0.11.0 | Valor |
| --- | --- |
| Contrato | [`CCMUYU5MMWGSQY7XF4W3ND2CIGXZZSDTRSDN23J4756CS6IRSR5MTJDZ`](https://stellar.expert/explorer/testnet/contract/CCMUYU5MMWGSQY7XF4W3ND2CIGXZZSDTRSDN23J4756CS6IRSR5MTJDZ) |
| WASM (sha256) | `13d7433d0ae1f3ff25a28fa9021848c42dd8332e8e81585111acbdd31c7a78f1` (76,228 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.5.0 |
| `buy_global_pass` | [`79f0b944…`](https://stellar.expert/explorer/testnet/tx/79f0b9447ea1580e17eab14088cdf1a0be7e8adccb69407930af7c0cb3f87ab1): un lector paga 2.00 USDC por un pase de 120 s. El USDC pasa al contrato |
| `use_pass` (soporte) | [`b2fb49ea…`](https://stellar.expert/explorer/testnet/tx/b2fb49ea75328ae4bb468cf327015e44777adcbb640122a085606a5b9cd1576c) y tres más: el operador anota 3 lecturas de la editorial demo y 1 de una segunda editorial ([`261651e3…`](https://stellar.expert/explorer/testnet/tx/261651e3270794ff369e9cf58cf2fa8b2fe01cd845ea4886c125bdcf41a6858b)). No se mueve nada |
| `cancel_pass` | Otro lector compra un pase de 1.00 ([`815ddaf9…`](https://stellar.expert/explorer/testnet/tx/815ddaf9daf4b2b2d5718d312574e8ffd7b71b55d02a9be8f812228a70d7e508)) y, sin leer nada, lo cancela: [`e1284a02…`](https://stellar.expert/explorer/testnet/tx/e1284a02885e3a8a8b02745cdae567a3d0b04a073c704f89568b8a70b5f9b6ca), recupera 1.00 |
| `settle_pass` | [`268e4d4f…`](https://stellar.expert/explorer/testnet/tx/268e4d4f9f5cef7a0b9921c93bb67716b7a21053b8b816e082d92f8b3c565fa2): vencido el pase, **otra cuenta** lo liquida, sin firma de quien lee. 0.04 a la tesorería; de los 1.96 restantes, 1.47 a la editorial demo (3 lecturas) y 0.49 a la segunda (1 lectura) |

Antes de enviar, el script simuló lo que no debe pasar: un segundo pase (#45), quien lee anotándose lecturas (#26), liquidar antes de vencer (#48), cancelar con lecturas (#49), anotar en un pase vencido (#46) y liquidar dos veces (#44). Al final, el contrato tiene saldo 0.

| v0.10.0 | Valor |
| --- | --- |
| Contrato | [`CCGPAGLTT7WMVSIESZXPB2AF72I2DYZHMI42VJEPOIL726JM3HKRC4WH`](https://stellar.expert/explorer/testnet/contract/CCGPAGLTT7WMVSIESZXPB2AF72I2DYZHMI42VJEPOIL726JM3HKRC4WH) |
| WASM (sha256) | `c15dcce77127f2cb25e3f27830aaea1597cdb68a9837b44b5145434821638887` (67,396 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.5.0 |
| `open_tab` | [`f718340c…`](https://stellar.expert/explorer/testnet/tx/f718340c316cdf6307844b276cdf6f140ed7670c5cfc881b67653f868c7f6b97): un lector deja 1.00 USDC de garantía, con 1 h para pagar |
| `charge_tab` (soporte) | [`319c8e3d…`](https://stellar.expert/explorer/testnet/tx/319c8e3db2487db3eac13febf778947c4672d0eefed4d47d08027568442c7638) y [`ab76abd6…`](https://stellar.expert/explorer/testnet/tx/ab76abd6d766e7244e90615ff98278c22fca1858ad57aa12a2772149dc9f975d): el operador anota 0.50 con la editorial demo y 0.30 con una segunda editorial. No se mueve nada |
| `settle_tab` | [`44490c5d…`](https://stellar.expert/explorer/testnet/tx/44490c5d758ab2e241e1538dd72a96742757de673f01677742d609ac90d11977): el lector paga 0.80 con una firma. 0.49 a la editorial demo, 0.294 a la segunda y 0.016 a la tesorería. La garantía sigue completa |
| `close_tab` | [`2d1edf5e…`](https://stellar.expert/explorer/testnet/tx/2d1edf5ebcf6a6fd1e99edc0033bc63cdaf8bf4f75ffbf79fc607953427e825c): sin deuda, la garantía de 1.00 vuelve a su wallet |
| `collect_tab` | Otro lector abre una cuenta con 60 s para pagar ([`2d4eb23e…`](https://stellar.expert/explorer/testnet/tx/2d4eb23e476d2c1b90ad3b4ad030c5bce32216bedbfdce676e8ff92d0ea8fc49)), se le anota 0.50 ([`782c726f…`](https://stellar.expert/explorer/testnet/tx/782c726f57fe735a070ca75bf508dadf5ae3f05aa2c6f51c11f9fb71e2f10fd0)) y no paga. Vencido el plazo, [`8a816cc1…`](https://stellar.expert/explorer/testnet/tx/8a816cc19e1fa8edb790ff03f310d40a0178b94dee9b9cdf640b292b85eaade0): **otra cuenta** cobra la deuda de la garantía, sin firma de quien lee. Le quedan 0.50 de garantía, que recupera al cerrar ([`0ef7027a…`](https://stellar.expert/explorer/testnet/tx/0ef7027a70ec22ffc137f6d09ffa0bcab6445e8364d9bf6b68ae15cd5f8b5fce)) |

Antes de enviar, el script simuló lo que no debe pasar: una segunda cuenta (#39), quien lee anotándose lecturas (#26), deuda mayor que la garantía (#40), cobrar de la garantía antes del plazo (#42), cerrar con deuda (#43) y pagar sin deuda (#41). Al final, el contrato tiene saldo 0.

| v0.9.0 | Valor |
| --- | --- |
| Contrato | [`CCGNE2XUYN2FNRE3EBE3HSNS7Q3EW4CTISUR6RYBUWQ7ACWZGI4GMYGB`](https://stellar.expert/explorer/testnet/contract/CCGNE2XUYN2FNRE3EBE3HSNS7Q3EW4CTISUR6RYBUWQ7ACWZGI4GMYGB) |
| WASM (sha256) | `9351005ee73683c99b2344df052e0dabe72e891e303a1ddbd42f52f13c6e1ab4` (58,549 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.5.0 |
| `buy_vouchers` | [`92e5cbeb…`](https://stellar.expert/explorer/testnet/tx/92e5cbebf39baaaaaa72711ba629912bc262c849fc048ddf4178c7a8d6068e1b): 2 vales de 0.50 USDC para el catálogo de la editorial demo, vigentes 150 s. Las llaves se generaron con `openssl` (ed25519); 1.00 USDC pasa al contrato |
| `redeem_voucher` | [`6f469d80…`](https://stellar.expert/explorer/testnet/tx/6f469d80a927a441185960043b60954c6c65a9940ba461224734accc1bc6851d): la envía **otra cuenta**, sin firma de quien compró ni de quien lee. El lector es una llave que no existe en la red. La firma del vale se hizo con `openssl` sobre el mensaje que arma `stellar xdr encode`. 0.49 a la editorial demo y 0.01 a la tesorería |
| `reclaim_vouchers` | [`f9ce75a6…`](https://stellar.expert/explorer/testnet/tx/f9ce75a6f3f8cea6732958b3798600b0f93290a2a4efac22ea4e914baa50f9d5): el segundo vale venció sin usarse y sus 0.50 vuelven a quien lo compró |

Antes de enviar, el script simuló lo que no debe pasar: la misma llave en otra compra (#37), la firma usada para otro lector, otro artículo u otro vale (las rechaza el host), canjear dos veces (#34), devolver antes de vencer (#36) y canjear un vale vencido (#35). Al final, el contrato tiene saldo 0.

| v0.8.0 | Valor |
| --- | --- |
| Contrato | [`CDUFLVEYPPGHJZTUVPW45YAAB7JAEKDKVZCYAQPKQBVMWETKFB2VPNIF`](https://stellar.expert/explorer/testnet/contract/CDUFLVEYPPGHJZTUVPW45YAAB7JAEKDKVZCYAQPKQBVMWETKFB2VPNIF) |
| WASM (sha256) | `d90b7c60644a85fbd593109aaa5b10c71aa85d1b3f0e37b4c55682135798faa5` (51,733 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.5.0 |
| `post_bounty` | [`2940ac9f…`](https://stellar.expert/explorer/testnet/tx/2940ac9f0a5c8ea4b1bd83513d7f5bc3afbc61e729ea471287d96827d505f1bd): 1.00 USDC en garantía por reproducir `crispr-nanomedicine-biotech`, con plazo de 1 h (recompensa 0) |
| `award_bounty` | [`4d804f94…`](https://stellar.expert/explorer/testnet/tx/4d804f948709a332b3c3f5a66300573d541e2aea41b2a977ca72843a663e6f72): 60/40 entre dos ganadores. El primero recibe 0.588; el segundo **no tiene trustline**, así que sus 0.392 quedan pendientes y los cobra con [`claim` `b592f3fe…`](https://stellar.expert/explorer/testnet/tx/b592f3fe20b9d1fdeb46eb164d11255c3c8e9a2b99a03b343823feb067178e56). 0.02 a la tesorería |
| `cancel_bounty` | Recompensa 1 de 0.30 con plazo de 90 s ([`eb576dc3…`](https://stellar.expert/explorer/testnet/tx/eb576dc38398f887231c9c02f7f2152097291e48c5b9e8c48972ee24409634db)). Vencida sin ganadores, [`2552af23…`](https://stellar.expert/explorer/testnet/tx/2552af234b4969c5fe093cca23ace48fe9e99765a382eb7c63d0ed2c18862184): vuelve completa a quien la publicó |

Antes de enviar, el script simuló lo que no debe pasar: cancelar antes del plazo (#32), otra cuenta repartiendo (#30) y repartir dos veces (#31). Al final, el contrato tiene saldo 0.

| v0.7.0 | Valor |
| --- | --- |
| Contrato | [`CDOI4FARQOJN6AXPWZQPZN5EBO47JY275L6VRIE3DP2NC35JIAGRDK3N`](https://stellar.expert/explorer/testnet/contract/CDOI4FARQOJN6AXPWZQPZN5EBO47JY275L6VRIE3DP2NC35JIAGRDK3N) |
| WASM (sha256) | `1cbe78d2826c6ae544b04444eb20e7a8e8b1abae4562b36fb8cc37b320e6ecb1` (45,518 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.5.0 |
| `fund_pool` | [`57cecee9…`](https://stellar.expert/explorer/testnet/tx/57cecee989c7932615b90ed8c474e523cea9607ce084ac607145977a5660ba4a): un patrocinador deposita 1.50 USDC para lecturas de la editorial demo, con tope de 0.50 y un operador. El USDC pasa al contrato |
| `pay_sponsored` | [`de1651fe…`](https://stellar.expert/explorer/testnet/tx/de1651fe1aa671f909a5f85dd1db85fd821618408f5258dfa66b28e0c603ad6e): el operador paga `crispr-nanomedicine-biotech` para una llave que **no existe en la red**. 0.49 a la editorial demo y 0.01 a la tesorería, desde el contrato. Eventos `paid` y `sponsored` |
| `withdraw_pool` | [`ea22d0b9…`](https://stellar.expert/explorer/testnet/tx/ea22d0b94ac3ba5cd95e3a4fa01f4d3a8fe0db510a04f4488ad13ec3a6c6e3c4): el patrocinador retira 1.00, lo que no se usó |

Antes de cada paso, el script simuló lo que no debe pasar: otra cuenta como operador (#26), más que el tope (#27), la misma lectura otra vez (#29), el operador retirando (#25) y un retiro mayor que el saldo (#28). Al final, el contrato tiene saldo 0.

| v0.6.0 | Valor |
| --- | --- |
| Contrato | [`CANRPEZT2DUNB3IP6ZDFDMAOH2H5R4AFPYRLYEMLTN4EE6O4QYSS6QM7`](https://stellar.expert/explorer/testnet/contract/CANRPEZT2DUNB3IP6ZDFDMAOH2H5R4AFPYRLYEMLTN4EE6O4QYSS6QM7) |
| WASM (sha256) | `b9e66f54e6e88f65d34250262803f574fed86b762b4eecc7b085d08e6e13c3df` (39,403 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.5.0 |
| `open_campaign` (soporte) | [`6993e953…`](https://stellar.expert/explorer/testnet/tx/6993e9533f98516b8b70d55927b999d0cb39ba8f111f020686652775b34866f1): una editorial nueva, **sin trustline de USDC**, ofrece liberar `crispr-nanomedicine-biotech` por 1.00 USDC con plazo de 1 h (campaña 0) |
| `pledge` | [`2498a459…`](https://stellar.expert/explorer/testnet/tx/2498a4590283b59bb704694f140fea2471c20a7f89d2fd524329237ba97ea189): 0.60 en garantía (el USDC pasa al contrato). Después, [`b1a2bd6f…`](https://stellar.expert/explorer/testnet/tx/b1a2bd6f7a8c26e757470e893469b25bbb4d04a43e8a109eb03d44346339cc31): otra cuenta aporta 0.40 y completa la meta |
| `release_pledges` | [`52040846…`](https://stellar.expert/explorer/testnet/tx/5204084632dc76ef9107b0cbdde478df45f0ecb7a9a1aea39dec86e324c74010): la envía una tercera cuenta, sin firma de nadie más. 0.02 a la tesorería; la editorial no puede recibir, así que sus 0.98 quedan pendientes a su nombre (`credited`) y se emite `released` |
| `claim` | [`cd62df31…`](https://stellar.expert/explorer/testnet/tx/cd62df31632cef23b99ca2d7cc8013717f947a0843392e1bdc5530d0594f19f8): la editorial nueva firma una vez. En la misma transacción se crea su trustline de USDC y recibe 0.98 |
| `refund_pledge` | Campaña 1 con plazo de 90 s y un aporte de 0.30 ([`a0e7f9d9…`](https://stellar.expert/explorer/testnet/tx/a0e7f9d9eec27331f91e37fc0379f30b25fc1e7e65a665c8ba8422bddeea20e1)). Vencida sin la meta, [`acdbb11b…`](https://stellar.expert/explorer/testnet/tx/acdbb11b6d7f32ade0f92abe75c33f4d426fc982f9b1ec23a4711d1e558b4706) la envía otra cuenta y los 0.30 vuelven completos a quien aportó (`unpledged`) |

Al final, el contrato tiene saldo 0, la tesorería recibió 0.02 y un segundo `claim` falla con `NothingToClaim` (#22). Las cuentas son desechables (Friendbot) y ninguna tiene poder sobre el contrato.

| v0.5.0 | Valor |
| --- | --- |
| Contrato | [`CA2QHBXMF6KIBBFT5ATQYMFDP75246RQV6NC6YA55QJZX3CRVFUE4RT2`](https://stellar.expert/explorer/testnet/contract/CA2QHBXMF6KIBBFT5ATQYMFDP75246RQV6NC6YA55QJZX3CRVFUE4RT2) |
| WASM (sha256) | `913ac9b4b43766bcc60e685d6a985e82dad677dc17d2c7f6a6aa892ca7b3bd17` (28,642 bytes), igual al que se baja de la red |
| Constructor | El de v0.2.0 más `pass_day_price` = 10000000 (1 USDC por día de pase) y `open_day_price` = 50000000 (5 USDC por día abierto) |
| `buy_pass` | [`747d2015…`](https://stellar.expert/explorer/testnet/tx/747d20154da627c210dde94b99a99b45042c98fb8985aa34cbbc69d4257b57a8): un lector le regala a otra cuenta un pase de 1 día al catálogo de la editorial demo (1.00 USDC). 0.98 a la editorial demo, 0.02 a la tesorería y un evento `pass` con esa cuenta como `reader` |
| `subscribe` | [`e4b2b08d…`](https://stellar.expert/explorer/testnet/tx/e4b2b08dc02e763d27be68bef87e3947d4446fc3cd8f903585bc320fe5f6b549): suscripción de 1 día por periodo con 2 renovaciones. Cobra el primer día (0.98 / 0.02) y, con la misma firma, autoriza al contrato por 2.00 USDC. Eventos `pass` y `subscribed` |
| `renew_subscription` | [`3f586bef…`](https://stellar.expert/explorer/testnet/tx/3f586befa2582e47633812238990ab691facb51e22c5c47b21fb8395a18aeff5): la envía **otra cuenta, sin USDC**; el lector no firma. 0.98 / 0.02 desde la wallet del lector, el pase llega a 2 días y queda 1 renovación (autorización en 1.00 USDC). Eventos `pass` y `renewed` |
| `sponsor_open` | [`a49b8687…`](https://stellar.expert/explorer/testnet/tx/a49b86873d7cc33103787f9b7af5cb474469efc2494f6a4594cc437f87b313fc): un patrocinador abre `crispr-nanomedicine-biotech` para todos 1 día (5.00 USDC). 4.90 a la editorial demo, 0.10 a la tesorería y un evento `opened` |

Después de las cuatro, el contrato tiene saldo 0, y una renovación adelantada falla con `RenewTooEarly` (#16). Las cuentas son desechables (Friendbot) y ninguna tiene poder sobre el contrato.

| v0.4.0 | Valor |
| --- | --- |
| Contrato | [`CC5SIM7JV6TXMWABU7G3DAA3V7HROFBOBF7I5TW4JZHKAQPBFJUWADGA`](https://stellar.expert/explorer/testnet/contract/CC5SIM7JV6TXMWABU7G3DAA3V7HROFBOBF7I5TW4JZHKAQPBFJUWADGA) |
| WASM (sha256) | `b97fffc81b053f597aaff0cb39b69a21005835ba412b8d8cf51681db1a6135aa` (15,339 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.2.0 |
| `pay_authors` | [`23a68ac1…`](https://stellar.expert/explorer/testnet/tx/23a68ac1be070324569dcb1a39b0e9ff1a80283a4809e63112ca0a3e648a4867): 0.50 USDC por un artículo con 3 coautores (50/30/20). 0.245 a la editorial demo, 0.147 y 0.098 a los otros dos, 0.01 a la tesorería y un evento `paid` a nombre del primer autor |
| `pay_ref` | [`ab2ca691…`](https://stellar.expert/explorer/testnet/tx/ab2ca6914b583415bf821e3223b651a4ab2d73863c827eb0634fb48752f7aa22): 0.50 USDC recomendado por otra cuenta. 0.49 a la editorial demo, 0.005 a quien recomendó, 0.005 a la tesorería y los eventos `paid` y `referred` |
| `pay_ref` desde el sitio | [`d3a130f6…`](https://stellar.expert/explorer/testnet/tx/d3a130f6aff66928b9d4c54288901d0507d61b5995918753bfe83086754d1197): el primero por `paperpay.blockchains.tf` (Pages Function y API de la VPS). Mismo reparto, 0.49 / 0.005 / 0.005 |

| v0.3.0 | Valor |
| --- | --- |
| Contrato | [`CCQRUTXBSWOSOHIZIEJ637HIMJZ6CZN5ZYSMTPGBTSMFAEXY6O5QHEQD`](https://stellar.expert/explorer/testnet/contract/CCQRUTXBSWOSOHIZIEJ637HIMJZ6CZN5ZYSMTPGBTSMFAEXY6O5QHEQD) |
| WASM (sha256) | `2af32e3a2017dedc6105c13e161a4a04bdad328c0ee271edbb2bad1d712318fb` (10,635 bytes), igual al que se baja de la red |
| Constructor | El mismo de v0.2.0 |
| `gift_batch` | [`70d30919…`](https://stellar.expert/explorer/testnet/tx/70d309192251ee041afadbbd51cd938c0fcab28fec3b258560241204544d07f9): 2 artículos (1.00 USDC) para una cuenta que no existe en la red. 0.98 a la editorial demo en una sola transferencia, 0.02 a la tesorería y 2 eventos `paid` con esa cuenta como `reader` |
| `tip` | [`36e14727…`](https://stellar.expert/explorer/testnet/tx/36e14727c832a99a448d80cece792899aa117599a5b301eadde96722434ed5d5): propina de 0.50 USDC a la editorial demo. 0.49 / 0.01 y un evento `tipped`, ningún `paid` |
| `refund` | [`3bc22905…`](https://stellar.expert/explorer/testnet/tx/3bc229057ea0312ea574dfee50cde01280a7fb647fc58cc5a8d2d37e1c0910db): una editorial desechable devuelve 0.49 USDC del [pago `a10772f8…`](https://stellar.expert/explorer/testnet/tx/a10772f8faa8ab883fba19824caadc69412378b92396d6b6028c30745b9dea3e). Evento `refunded`; la comisión de 0.01 se queda en la tesorería |

Las cuentas de esta prueba son desechables (Friendbot): sus llaves se borraron al terminar y ninguna tiene poder sobre el contrato.

**v0.2.0 (en producción del 23 al 25 de noviembre; hoy, contrato anterior):**

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

Desplegado en testnet y **activo** desde el 23 de noviembre de 2026 en la API de la VPS y en [paperpay.blockchains.tf](https://paperpay.blockchains.tf/): con Freighter, cada lectura paga por `pay`. Prueba de punta a punta contra el sitio publicado: [`96dd614e…`](https://stellar.expert/explorer/testnet/tx/96dd614e5a3b13c2e0d7dfc58d1e5b6457f554dc22754e25ce89a719293e74d9), 0.49 a la editorial y 0.01 a la tesorería. Desde el 24 de noviembre, el sitio también ofrece `gift` ("Regálalo"): quien regala paga y comparte un enlace con el que otra persona lee sin wallet (ver [la API](../apps/api/README.md#regalo-por-enlace-gift)); prueba contra el sitio publicado: [`3fe887ee…`](https://stellar.expert/explorer/testnet/tx/3fe887ee932f4e260267dc80b4a2a7c5d31ecf1d6b776a7eea7bc6931d517f7e). Pollar y el sitio de Vercel siguen con transferencias completas a la tesorería. `pay_batch` funciona en la red, pero el sitio todavía no lo ofrece.

## `boveda`: tarjeta de regalo con código

Guarda XLM y lo entrega completo a quien presente el código. Es la bóveda del reto exprés de Ship It (26 de noviembre de 2026, 12:00–16:00 CDMX) y, en PaperPay, la tarjeta de regalo de lectura: el regalo por enlace regala un artículo; la tarjeta regala saldo con un código. La pantalla es [`/boveda`](https://paperpay.blockchains.tf/boveda).

- **`__constructor(token, code_hash)`:** guarda el SAC del token y el sha256 del código, nunca el texto.
- **`deposit(from, amount) -> saldo`:** firma `from`; el monto tiene que ser mayor que 0 (`InvalidAmount`, 1). Emite `deposited` con el monto y el saldo.
- **`claim(claimer, code) -> monto`:** firma `claimer`. Si el sha256 del código no coincide, `WrongCode` (2); si no hay saldo, `Empty` (3). Si coincide, le transfiere todo el saldo y emite `claimed`.
- De lectura: `balance` y `config`.
- Sin admin ni retiro: los XLM solo salen con el código. Cada uso extiende el TTL al máximo.
- Pruebas: 10 (`cargo test -p paperpay-boveda`), con el árbol de autorización de `deposit` y `claim`, 6 códigos parecidos que fallan, bóveda vacía, relleno y TTL. Mutaciones: 13 de 13 detectadas.

La pantalla perdona mayúsculas, comillas y espacios de más («ship it» → `Ship It`); el contrato solo acepta el código exacto. Si la cuenta de quien cobra no existe en testnet, la pantalla ofrece activarla con Friendbot.

### En testnet

| Dato | Valor |
| --- | --- |
| Contrato (v0.1.0) | [`CAYVRBBV5CSX5K6AHXZHI3KJS72O7OVHSHWX27MYA5HNVTP4N5OHG6VS`](https://stellar.expert/explorer/testnet/contract/CAYVRBBV5CSX5K6AHXZHI3KJS72O7OVHSHWX27MYA5HNVTP4N5OHG6VS), creado el 26 de noviembre a las 12:00:05 CDMX ([`1ff98795…`](https://stellar.expert/explorer/testnet/tx/1ff98795eb35284646298f69bbcf563d83e461c189d2a276ff831ffed65559db)) |
| Constructor | token = SAC de XLM nativo de testnet (`CDLZFC3S…`), `code_hash` = `7121f6ba…` (sha256 de «Ship It») |
| WASM (sha256) | `9fbd26163cf8301cc4a1892e8c27a6e9c9090cd6530ea7dd6d4780de11cc9595`, igual al que se baja de la red con `stellar contract fetch` |
| `deposit` | [`fbf208f0…`](https://stellar.expert/explorer/testnet/tx/fbf208f031465f5bb158374ce65296b7394a27545989ec1c49757b874eec09ab): 5555 XLM a las 12:00:13 |
| `claim` | [`e0d3e3d7…`](https://stellar.expert/explorer/testnet/tx/e0d3e3d7fbadf27ad9ff3c16c1ce67d91b147094431ded52c0fe0a3d2ffda10e): una cuenta ajena a PaperPay (la revisión del reto) cobró los 5555 XLM a las 12:32:18 |
| Relleno | [`138328bb…`](https://stellar.expert/explorer/testnet/tx/138328bbf4054bd8311098e981f40b6cf79cbdf39b0689090479bead9d251560): otros 5555 XLM a las 12:32:26, de una vigilancia que corrió hasta las 16:00 |

Los datos completos están en [`deployments/testnet.json`](deployments/testnet.json) (`boveda`).
