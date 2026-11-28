#![no_std]

//! Mecenas: dos formas de pagar por otros en PaperPay. Vive en un contrato aparte del reparto
//! (`splitter`), que llegó al tope de tamaño de la red; usa el mismo USDC, la misma tesorería y
//! la misma comisión (`fee_bps`, 200 = 2%).
//!
//! Mecenazgo por flujo: alguien financia a un autor o a un laboratorio poco a poco.
//! - `open_stream`: el mecenas deposita un monto que se devenga segundo a segundo durante un
//!   periodo.
//! - `withdraw_stream`: quien recibe retira lo devengado hasta ese momento (98%; 2% a la
//!   tesorería).
//! - `cancel_stream`: el mecenas corta el flujo y recupera lo que todavía no se devengó. Lo ya
//!   devengado sigue siendo de quien recibe.
//!
//! Lectura pendiente, como el café pendiente: alguien deja lecturas pagadas para quien las
//! necesite.
//! - `leave_read`: alguien paga por adelantado lecturas de una editorial, a un precio.
//! - `take_read`: un lector toma una con su firma, sin pagar; la editorial cobra en ese momento.
//!   Una por lector y editorial cada 24 h.
//!
//! El contrato guarda dos cosas y nada más: lo no retirado de cada flujo y las lecturas
//! pendientes. Cada una tiene sus salidas fijas. La configuración se fija al desplegar y no
//! cambia: no hay admin.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env, String,
};

/// 100% en puntos base.
pub const BPS_DENOMINATOR: i128 = 10_000;
/// Comisión máxima aceptada al desplegar (10%).
pub const MAX_FEE_BPS: u32 = 1_000;
/// Largo máximo del id de un artículo, en bytes.
pub const MAX_PAPER_ID_LEN: u32 = 64;
/// Segundos de un día, en la hora del ledger.
pub const DAY_SECONDS: u64 = 86_400;
/// Duración máxima de un flujo, en segundos (90 días).
pub const MAX_STREAM_SECONDS: u64 = 90 * DAY_SECONDS;
/// Lecturas pendientes que se pueden dejar en una llamada.
pub const MAX_READS_PER_GIFT: u32 = 50;

const DAY_IN_LEDGERS: u32 = 17_280;
const INSTANCE_TTL_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;
const INSTANCE_TTL_EXTEND_TO: u32 = 30 * DAY_IN_LEDGERS;
// Un ledger cada ~5 s. Un flujo vive en la red hasta que termina, 30 días más para retirar y una
// semana de margen.
const LEDGER_SECONDS: u64 = 5;
const GRACE_SECONDS: u64 = 30 * DAY_SECONDS;
const TTL_MARGIN_LEDGERS: u32 = 7 * DAY_IN_LEDGERS;
/// Cuánto dura el marcador de "ya tomó una": un día de ledgers (~24 h).
const TAKEN_TTL_LEDGERS: u32 = DAY_IN_LEDGERS;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// `fee_bps` mayor a `MAX_FEE_BPS` al desplegar.
    FeeTooHigh = 1,
    /// Monto o precio menor o igual a cero.
    InvalidAmount = 2,
    /// Id de artículo vacío o de más de `MAX_PAPER_ID_LEN` bytes.
    InvalidPaperId = 3,
    /// El monto no cabe en la cuenta que hay que hacer con él.
    AmountOverflow = 4,
    /// Duración de un flujo de cero o de más de `MAX_STREAM_SECONDS`.
    InvalidDuration = 5,
    /// No hay flujo con ese id (o esa cuenta no es parte de él).
    NoStream = 6,
    /// Todavía no hay nada devengado sin retirar.
    NothingToWithdraw = 7,
    /// El flujo ya terminó o ya se canceló: no queda nada por cortar.
    StreamEnded = 8,
    /// Cantidad de lecturas de cero o de más de `MAX_READS_PER_GIFT`.
    InvalidCount = 9,
    /// No hay lecturas pendientes de esa editorial a ese precio.
    NoPendingRead = 10,
    /// Ese lector ya tomó una lectura pendiente de esa editorial en las últimas 24 h.
    AlreadyTaken = 11,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    /// Contrato del token con el que se paga (el SAC de USDC).
    pub token: Address,
    /// Cuenta que recibe la comisión.
    pub treasury: Address,
    /// Comisión en puntos base (200 = 2%).
    pub fee_bps: u32,
}

/// Cómo se reparte un pago: `net` va a quien cobra y `fee`, a la tesorería.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Split {
    pub net: i128,
    pub fee: i128,
}

/// Un flujo: `patron` dejó `amount` en el contrato para `beneficiary`, que se devenga de forma
/// pareja entre `start` y `end` (segundos Unix). `withdrawn` es lo que ya se retiró. Al
/// cancelar, `amount` baja a lo devengado y `end` pasa a ser el momento del corte.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stream {
    pub patron: Address,
    pub beneficiary: Address,
    pub amount: i128,
    pub start: u64,
    pub end: u64,
    pub withdrawn: i128,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Config,
    /// Id del próximo flujo (instance): los ids son secuenciales desde 0.
    NextStream,
    /// Un flujo, por id.
    Stream(u32),
    /// Cuántas lecturas pendientes hay de `publisher` (`.0`) al precio `.1`.
    Pending(Address, i128),
    /// Marcador temporal: `reader` (`.0`) ya tomó una lectura pendiente de `publisher` (`.1`).
    /// Vence solo en `TAKEN_TTL_LEDGERS`.
    Taken(Address, Address),
}

/// Un flujo nuevo. Topics: `["stream_opened", patron, beneficiary]`. `amount` ya está en el
/// contrato y se devenga entre `start` y `end`.
#[contractevent(topics = ["stream_opened"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamOpened {
    #[topic]
    pub patron: Address,
    #[topic]
    pub beneficiary: Address,
    pub stream_id: u32,
    pub amount: i128,
    pub start: u64,
    pub end: u64,
}

/// Un retiro de lo devengado. Topics: `["stream_withdrawn", beneficiary]`. `amount` es lo que
/// salió del flujo (con la comisión `fee` incluida) y `withdrawn`, lo retirado en total.
#[contractevent(topics = ["stream_withdrawn"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamWithdrawn {
    #[topic]
    pub beneficiary: Address,
    pub stream_id: u32,
    pub amount: i128,
    pub fee: i128,
    pub withdrawn: i128,
}

/// Un flujo cortado. Topics: `["stream_cancelled", patron]`. `refund` volvió a `patron` y `kept`
/// es lo ya devengado, que sigue siendo de quien recibe.
#[contractevent(topics = ["stream_cancelled"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamCancelled {
    #[topic]
    pub patron: Address,
    pub stream_id: u32,
    pub refund: i128,
    pub kept: i128,
}

/// Lecturas dejadas pagadas. Topics: `["reads_left", donor, publisher]`. `pending` es cuántas hay
/// ahora de esa editorial a ese precio.
#[contractevent(topics = ["reads_left"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadsLeft {
    #[topic]
    pub donor: Address,
    #[topic]
    pub publisher: Address,
    pub price: i128,
    pub count: u32,
    pub pending: u32,
}

/// Una lectura pendiente tomada: el comprobante de acceso de `reader` a `paper_id`. Topics:
/// `["read_taken", publisher, reader]`. `amount` es el precio que cobró la editorial (con la
/// comisión `fee` incluida).
#[contractevent(topics = ["read_taken"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadTaken {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub paper_id: String,
    pub amount: i128,
    pub fee: i128,
}

#[contract]
pub struct Mecenas;

#[contractimpl]
impl Mecenas {
    /// Fija el token, la tesorería y la comisión. No hay admin: para cambiarlos se despliega
    /// otro contrato.
    pub fn __constructor(env: Env, token: Address, treasury: Address, fee_bps: u32) {
        if fee_bps > MAX_FEE_BPS {
            panic_with_error!(&env, Error::FeeTooHigh);
        }
        env.storage().instance().set(
            &DataKey::Config,
            &Config {
                token,
                treasury,
                fee_bps,
            },
        );
    }

    /// `patron` abre un flujo para `beneficiary` (un autor, un laboratorio): deposita `amount`,
    /// que pasa de su wallet al contrato y se devenga de forma pareja durante `duration`
    /// segundos (de 1 a `MAX_STREAM_SECONDS`). Ese dinero tiene dos salidas: lo devengado, hacia
    /// `beneficiary` (`withdraw_stream`); lo no devengado, de vuelta a `patron` si corta el flujo
    /// (`cancel_stream`). Solo firma `patron`. Emite `stream_opened` y devuelve el id del flujo.
    pub fn open_stream(
        env: Env,
        patron: Address,
        beneficiary: Address,
        amount: i128,
        duration: u64,
    ) -> u32 {
        patron.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        if duration == 0 || duration > MAX_STREAM_SECONDS {
            panic_with_error!(&env, Error::InvalidDuration);
        }
        // Lo devengado se calcula como `amount × segundos / duración`: el producto más grande
        // posible tiene que caber.
        if amount.checked_mul(duration as i128).is_none() {
            panic_with_error!(&env, Error::AmountOverflow);
        }

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &patron,
            env.current_contract_address(),
            &amount,
        );
        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextStream)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::NextStream, &(id + 1));
        let start = env.ledger().timestamp();
        let end = start + duration;
        let entry = DataKey::Stream(id);
        env.storage().persistent().set(
            &entry,
            &Stream {
                patron: patron.clone(),
                beneficiary: beneficiary.clone(),
                amount,
                start,
                end,
                withdrawn: 0,
            },
        );
        keep_until(&env, &entry, end + GRACE_SECONDS);
        extend_instance_ttl(&env);

        StreamOpened {
            patron,
            beneficiary,
            stream_id: id,
            amount,
            start,
            end,
        }
        .publish(&env);

        id
    }

    /// `beneficiary` retira todo lo devengado y no retirado de su flujo `stream_id`: recibe ese
    /// monto menos la comisión y la tesorería, la comisión. Se puede retirar cuantas veces se
    /// quiera, también después de que el flujo termine o se corte. Sin nada nuevo devengado,
    /// falla (`NothingToWithdraw`). Solo firma `beneficiary`. Emite `stream_withdrawn` y devuelve
    /// el reparto.
    pub fn withdraw_stream(env: Env, beneficiary: Address, stream_id: u32) -> Split {
        beneficiary.require_auth();
        let entry = DataKey::Stream(stream_id);
        let mut stream = read_stream(&env, &entry);
        if stream.beneficiary != beneficiary {
            panic_with_error!(&env, Error::NoStream);
        }
        let available = accrued(&stream, env.ledger().timestamp()) - stream.withdrawn;
        if available <= 0 {
            panic_with_error!(&env, Error::NothingToWithdraw);
        }
        stream.withdrawn += available;
        env.storage().persistent().set(&entry, &stream);

        let config = read_config(&env);
        let split = compute_split(&env, available, config.fee_bps);
        pay_from_custody(&env, &config, &beneficiary, &split);
        extend_instance_ttl(&env);

        StreamWithdrawn {
            beneficiary,
            stream_id,
            amount: available,
            fee: split.fee,
            withdrawn: stream.withdrawn,
        }
        .publish(&env);

        split
    }

    /// `patron` corta su flujo `stream_id`: lo que todavía no se devengó vuelve completo a su
    /// wallet, sin comisión, y el flujo queda terminado en ese momento. Lo ya devengado no vuelve:
    /// sigue en el contrato hasta que quien recibe lo retire. Un flujo que ya terminó no se corta
    /// (`StreamEnded`). Solo firma `patron`. Emite `stream_cancelled` y devuelve lo recuperado.
    pub fn cancel_stream(env: Env, patron: Address, stream_id: u32) -> i128 {
        patron.require_auth();
        let entry = DataKey::Stream(stream_id);
        let mut stream = read_stream(&env, &entry);
        if stream.patron != patron {
            panic_with_error!(&env, Error::NoStream);
        }
        let now = env.ledger().timestamp();
        if now >= stream.end {
            panic_with_error!(&env, Error::StreamEnded);
        }
        let kept = accrued(&stream, now);
        let refund = stream.amount - kept;
        stream.amount = kept;
        stream.end = now;
        env.storage().persistent().set(&entry, &stream);

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &patron,
            &refund,
        );
        extend_instance_ttl(&env);

        StreamCancelled {
            patron,
            stream_id,
            refund,
            kept,
        }
        .publish(&env);

        refund
    }

    /// El flujo `stream_id`, si existe.
    pub fn stream(env: Env, stream_id: u32) -> Option<Stream> {
        env.storage().persistent().get(&DataKey::Stream(stream_id))
    }

    /// `donor` deja pagadas `count` lecturas (de 1 a `MAX_READS_PER_GIFT`) de `publisher`, a
    /// `price` cada una, para quien las necesite: el total pasa de su wallet al contrato. No
    /// elige quién las lee ni las puede recuperar: es un regalo. Ese dinero solo sale hacia esa
    /// editorial (y la comisión), una lectura a la vez (`take_read`). Solo firma `donor`. Emite
    /// `reads_left` y devuelve cuántas quedan pendientes de esa editorial a ese precio.
    pub fn leave_read(
        env: Env,
        donor: Address,
        publisher: Address,
        price: i128,
        count: u32,
    ) -> u32 {
        donor.require_auth();
        if price <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        if count == 0 || count > MAX_READS_PER_GIFT {
            panic_with_error!(&env, Error::InvalidCount);
        }
        let total = match price.checked_mul(count as i128) {
            Some(total) => total,
            None => panic_with_error!(&env, Error::AmountOverflow),
        };
        let entry = DataKey::Pending(publisher.clone(), price);
        let before: u32 = env.storage().persistent().get(&entry).unwrap_or(0);
        let pending = match before.checked_add(count) {
            Some(pending) => pending,
            None => panic_with_error!(&env, Error::AmountOverflow),
        };

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &donor,
            env.current_contract_address(),
            &total,
        );
        env.storage().persistent().set(&entry, &pending);
        keep_max(&env, &entry);
        extend_instance_ttl(&env);

        ReadsLeft {
            donor,
            publisher,
            price,
            count,
            pending,
        }
        .publish(&env);

        pending
    }

    /// `reader` toma una lectura pendiente de `publisher` a `price` para leer `paper_id`: no
    /// paga nada. La editorial recibe el precio menos la comisión y la tesorería, la comisión,
    /// desde lo que alguien dejó pagado. Una por lector y editorial cada 24 h (`AlreadyTaken`),
    /// para que alcancen para más gente. Como en `pay` del contrato de reparto, que el precio
    /// sea el del artículo lo valida quien da el acceso, contra el evento. Solo firma `reader`.
    /// Emite `read_taken` y devuelve el reparto.
    pub fn take_read(
        env: Env,
        reader: Address,
        publisher: Address,
        paper_id: String,
        price: i128,
    ) -> Split {
        reader.require_auth();
        if paper_id.is_empty() || paper_id.len() > MAX_PAPER_ID_LEN {
            panic_with_error!(&env, Error::InvalidPaperId);
        }
        let entry = DataKey::Pending(publisher.clone(), price);
        let pending: u32 = env.storage().persistent().get(&entry).unwrap_or(0);
        if pending == 0 {
            panic_with_error!(&env, Error::NoPendingRead);
        }
        let marker = DataKey::Taken(reader.clone(), publisher.clone());
        if env.storage().temporary().has(&marker) {
            panic_with_error!(&env, Error::AlreadyTaken);
        }
        env.storage().temporary().set(&marker, &true);
        env.storage()
            .temporary()
            .extend_ttl(&marker, TAKEN_TTL_LEDGERS, TAKEN_TTL_LEDGERS);
        if pending == 1 {
            env.storage().persistent().remove(&entry);
        } else {
            env.storage().persistent().set(&entry, &(pending - 1));
            keep_max(&env, &entry);
        }

        let config = read_config(&env);
        let split = compute_split(&env, price, config.fee_bps);
        pay_from_custody(&env, &config, &publisher, &split);
        extend_instance_ttl(&env);

        ReadTaken {
            publisher,
            reader,
            paper_id,
            amount: price,
            fee: split.fee,
        }
        .publish(&env);

        split
    }

    /// Cuántas lecturas pendientes hay de `publisher` a `price`.
    pub fn pending_reads(env: Env, publisher: Address, price: i128) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Pending(publisher, price))
            .unwrap_or(0)
    }

    /// La configuración fijada al desplegar.
    pub fn config(env: Env) -> Config {
        read_config(&env)
    }
}

/// Lo devengado de un flujo en `now`: todo si ya terminó; si no, la parte proporcional al tiempo
/// transcurrido, redondeada hacia abajo (lo que sobra se devenga al final).
fn accrued(stream: &Stream, now: u64) -> i128 {
    if now >= stream.end {
        return stream.amount;
    }
    // `now < end` implica `end > start`. El producto cabe: se comprobó al abrir, con la duración
    // completa, y al cortar el monto y la duración solo bajan.
    let elapsed = (now - stream.start) as i128;
    let duration = (stream.end - stream.start) as i128;
    stream.amount * elapsed / duration
}

/// Paga un reparto desde la custodia del contrato: `net` a `to` y `fee` a la tesorería (omite
/// montos en cero).
fn pay_from_custody(env: &Env, config: &Config, to: &Address, split: &Split) {
    let usdc = token::Client::new(env, &config.token);
    let from = env.current_contract_address();
    if split.net > 0 {
        usdc.transfer(&from, to, &split.net);
    }
    if split.fee > 0 {
        usdc.transfer(&from, &config.treasury, &split.fee);
    }
}

fn read_stream(env: &Env, entry: &DataKey) -> Stream {
    match env.storage().persistent().get(entry) {
        Some(stream) => stream,
        None => panic_with_error!(env, Error::NoStream),
    }
}

fn read_config(env: &Env) -> Config {
    env.storage().instance().get(&DataKey::Config).unwrap()
}

fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND_TO);
}

/// Extiende el TTL de una entrada persistente para que siga en la red hasta `until` (segundos
/// Unix) y `TTL_MARGIN_LEDGERS` más, sin pasar del máximo de la red.
fn keep_until(env: &Env, key: &DataKey, until: u64) {
    let secs = until.saturating_sub(env.ledger().timestamp());
    let ledgers = (secs / LEDGER_SECONDS) as u32 + TTL_MARGIN_LEDGERS;
    let ttl = ledgers.min(env.storage().max_ttl());
    env.storage().persistent().extend_ttl(key, ttl, ttl);
}

/// Lleva al TTL máximo de la red una entrada persistente sin fecha de vencimiento.
fn keep_max(env: &Env, key: &DataKey) {
    let max = env.storage().max_ttl();
    env.storage().persistent().extend_ttl(key, max, max);
}

/// La comisión se redondea hacia abajo: cualquier fracción de stroop queda para quien cobra.
fn compute_split(env: &Env, amount: i128, fee_bps: u32) -> Split {
    if amount <= 0 {
        panic_with_error!(env, Error::InvalidAmount);
    }
    let fee = match amount.checked_mul(fee_bps as i128) {
        Some(scaled) => scaled / BPS_DENOMINATOR,
        None => panic_with_error!(env, Error::AmountOverflow),
    };
    Split {
        net: amount - fee,
        fee,
    }
}

#[cfg(test)]
mod test;
