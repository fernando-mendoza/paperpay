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
//! Desde la v0.2.0:
//! - `pay_from_stream`: quien recibe un flujo paga una lectura con lo devengado, sin retirarlo.
//!
//! Fianza de integridad: una editorial respalda un artículo con su dinero.
//! - `post_bond`: la editorial deposita una fianza por un artículo y nombra a un árbitro (un
//!   comité de integridad), por un plazo.
//! - `slash_bond`: dentro del plazo, el árbitro manda parte o toda la fianza a quien corresponda
//!   (por ejemplo, a los lectores de un artículo retractado). Nunca a sí mismo.
//! - `release_bond`: pasado el plazo, la editorial recupera lo que quede.
//!
//! Apadrinar a un lector: alguien paga las lecturas de una persona concreta.
//! - `sponsor_reader`: el padrino deposita para las lecturas de un lector con una editorial.
//! - `read_sponsored`: ese lector paga una lectura con ese saldo, con su sola firma.
//! - `reclaim_sponsorship`: el padrino recupera lo que no se usó.
//!
//! Leer a medias: dos personas compran un artículo entre las dos.
//! - `open_shared`: una pone su mitad y nombra a la otra.
//! - `join_shared`: la otra pone su mitad; la editorial cobra y las dos tienen acceso.
//! - `cancel_shared`: quien abrió recupera su mitad mientras la otra no haya pagado.
//!
//! El contrato guarda lo no retirado de cada flujo, las lecturas pendientes, las fianzas, lo
//! apadrinado sin usar y las mitades de las lecturas a medias sin completar. Cada cosa tiene sus
//! salidas fijas. La configuración se fija al desplegar y no cambia: no hay admin.

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
/// Duración máxima de una fianza, en segundos (90 días).
pub const MAX_BOND_SECONDS: u64 = 90 * DAY_SECONDS;

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
    /// No hay nada (o no hay suficiente) devengado sin retirar, o no queda nada por recuperar.
    NothingToWithdraw = 7,
    /// El flujo ya terminó o ya se canceló: no queda nada por cortar.
    StreamEnded = 8,
    /// Cantidad de lecturas de cero o de más de `MAX_READS_PER_GIFT`.
    InvalidCount = 9,
    /// No hay lecturas pendientes de esa editorial a ese precio.
    NoPendingRead = 10,
    /// Ese lector ya tomó una lectura pendiente de esa editorial en las últimas 24 h.
    AlreadyTaken = 11,
    /// No hay fianza con ese id (o esa cuenta no es parte de ella).
    NoBond = 12,
    /// El plazo de la fianza ya terminó: lo que queda solo puede volver a la editorial.
    BondExpired = 13,
    /// El plazo de la fianza sigue corriendo: la editorial todavía no la recupera.
    BondActive = 14,
    /// La editorial no puede ser su propio árbitro, ni el árbitro pagarse a sí mismo.
    SelfArbiter = 15,
    /// A lo apadrinado para ese lector no le alcanza para ese pago.
    SponsorshipTooLow = 16,
    /// No hay lectura a medias con ese id (o esa cuenta no es parte de ella).
    NoShared = 17,
    /// La lectura a medias ya se completó o se canceló.
    SharedClosed = 18,
    /// Para leer a medias hacen falta dos cuentas distintas.
    SamePerson = 19,
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

/// Una fianza de integridad: `publisher` dejó `amount` en el contrato para respaldar `paper_id`.
/// Hasta `until` (segundos Unix), `arbiter` puede mandar parte o todo a quien corresponda; desde
/// `until`, lo que quede vuelve a `publisher`. `amount` es lo que queda.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bond {
    pub publisher: Address,
    pub arbiter: Address,
    pub paper_id: String,
    pub amount: i128,
    pub until: u64,
}

/// Una lectura a medias: `first` puso su mitad de `price` por `paper_id` de `publisher` y espera
/// la de `second`. `open` es falso cuando se completó o se canceló.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shared {
    pub first: Address,
    pub second: Address,
    pub publisher: Address,
    pub paper_id: String,
    pub price: i128,
    pub open: bool,
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
    /// Id de la próxima fianza (instance).
    NextBond,
    /// Una fianza, por id.
    Bond(u32),
    /// Lo que `sponsor` (`.0`) dejó para las lecturas de `reader` (`.1`) con `publisher` (`.2`).
    Sponsorship(Address, Address, Address),
    /// Id de la próxima lectura a medias (instance).
    NextShared,
    /// Una lectura a medias, por id.
    Shared(u32),
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

/// Una fianza nueva. Topics: `["bond_posted", publisher, arbiter]`. `amount` ya está en el
/// contrato hasta `until`.
#[contractevent(topics = ["bond_posted"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BondPosted {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub arbiter: Address,
    pub bond_id: u32,
    pub paper_id: String,
    pub amount: i128,
    pub until: u64,
}

/// Parte de una fianza ejecutada. Topics: `["bond_slashed", arbiter, to]`. `amount` pasó a `to` y
/// `remaining` es lo que queda de la fianza.
#[contractevent(topics = ["bond_slashed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BondSlashed {
    #[topic]
    pub arbiter: Address,
    #[topic]
    pub to: Address,
    pub bond_id: u32,
    pub amount: i128,
    pub remaining: i128,
}

/// Una fianza aumentada por su editorial. Topics: `["bond_topped_up", publisher]`. `total` es lo
/// que respalda ahora.
#[contractevent(topics = ["bond_topped_up"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BondToppedUp {
    #[topic]
    pub publisher: Address,
    pub bond_id: u32,
    pub amount: i128,
    pub total: i128,
}

/// Una fianza liberada por su árbitro antes del plazo. Topics: `["bond_cleared", arbiter,
/// publisher]`.
#[contractevent(topics = ["bond_cleared"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BondCleared {
    #[topic]
    pub arbiter: Address,
    #[topic]
    pub publisher: Address,
    pub bond_id: u32,
    pub amount: i128,
}

/// Honorarios pagados al árbitro de una fianza. Topics: `["arbiter_paid", publisher, arbiter]`.
/// `amount` incluye la comisión `fee`.
#[contractevent(topics = ["arbiter_paid"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbiterPaid {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub arbiter: Address,
    pub bond_id: u32,
    pub amount: i128,
    pub fee: i128,
}

/// Una fianza devuelta a la editorial al terminar su plazo. Topics: `["bond_released", publisher]`.
#[contractevent(topics = ["bond_released"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BondReleased {
    #[topic]
    pub publisher: Address,
    pub bond_id: u32,
    pub amount: i128,
}

/// Un depósito para las lecturas de un lector. Topics: `["reader_sponsored", sponsor, reader]`.
/// `balance` es lo que ese lector tiene ahora de ese padrino con esa editorial.
#[contractevent(topics = ["reader_sponsored"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReaderSponsored {
    #[topic]
    pub sponsor: Address,
    #[topic]
    pub reader: Address,
    pub publisher: Address,
    pub amount: i128,
    pub balance: i128,
}

/// Lo apadrinado que volvió al padrino sin usarse. Topics: `["sponsorship_reclaimed", sponsor,
/// reader]`.
#[contractevent(topics = ["sponsorship_reclaimed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SponsorshipReclaimed {
    #[topic]
    pub sponsor: Address,
    #[topic]
    pub reader: Address,
    pub publisher: Address,
    pub amount: i128,
}

/// Una lectura a medias abierta. Topics: `["shared_opened", first, second]`. `paid` es la mitad
/// que ya puso `first`.
#[contractevent(topics = ["shared_opened"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SharedOpened {
    #[topic]
    pub first: Address,
    #[topic]
    pub second: Address,
    pub shared_id: u32,
    pub publisher: Address,
    pub paper_id: String,
    pub price: i128,
    pub paid: i128,
}

/// Una lectura a medias completada: el comprobante de acceso de las dos cuentas. Topics:
/// `["shared_joined", publisher, first, second]`. La editorial cobró `price` menos `fee`.
#[contractevent(topics = ["shared_joined"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SharedJoined {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub first: Address,
    #[topic]
    pub second: Address,
    pub shared_id: u32,
    pub paper_id: String,
    pub price: i128,
    pub fee: i128,
}

/// Una lectura a medias cancelada. Topics: `["shared_cancelled", first]`. `amount` volvió a
/// `first`.
#[contractevent(topics = ["shared_cancelled"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SharedCancelled {
    #[topic]
    pub first: Address,
    pub shared_id: u32,
    pub amount: i128,
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

    /// `beneficiary` paga `amount` por `paper_id` de `publisher` con lo devengado de su flujo
    /// `stream_id`, sin retirarlo antes: la editorial recibe el monto menos la comisión y la
    /// tesorería, la comisión, desde el flujo. Si lo devengado sin retirar no alcanza, falla
    /// (`NothingToWithdraw`). Solo firma `beneficiary`. Emite `read_taken` y devuelve el reparto.
    pub fn pay_from_stream(
        env: Env,
        beneficiary: Address,
        stream_id: u32,
        publisher: Address,
        paper_id: String,
        amount: i128,
    ) -> Split {
        beneficiary.require_auth();
        validate_paper_id(&env, &paper_id);
        let entry = DataKey::Stream(stream_id);
        let mut stream = read_stream(&env, &entry);
        if stream.beneficiary != beneficiary {
            panic_with_error!(&env, Error::NoStream);
        }
        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);
        let available = accrued(&stream, env.ledger().timestamp()) - stream.withdrawn;
        if amount > available {
            panic_with_error!(&env, Error::NothingToWithdraw);
        }
        stream.withdrawn += amount;
        env.storage().persistent().set(&entry, &stream);

        pay_from_custody(&env, &config, &publisher, &split);
        extend_instance_ttl(&env);

        ReadTaken {
            publisher,
            reader: beneficiary,
            paper_id,
            amount,
            fee: split.fee,
        }
        .publish(&env);

        split
    }

    /// `publisher` respalda `paper_id` con una fianza: deposita `amount`, que queda en el
    /// contrato durante `duration` segundos (de 1 a `MAX_BOND_SECONDS`), y nombra a `arbiter`
    /// (por ejemplo, un comité de integridad), que no puede ser la propia editorial
    /// (`SelfArbiter`). Dentro del plazo solo el árbitro mueve ese dinero (`slash_bond`); pasado
    /// el plazo, lo que quede vuelve a la editorial (`release_bond`). Solo firma `publisher`.
    /// Emite `bond_posted` y devuelve el id de la fianza.
    pub fn post_bond(
        env: Env,
        publisher: Address,
        arbiter: Address,
        paper_id: String,
        amount: i128,
        duration: u64,
    ) -> u32 {
        publisher.require_auth();
        validate_paper_id(&env, &paper_id);
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        if duration == 0 || duration > MAX_BOND_SECONDS {
            panic_with_error!(&env, Error::InvalidDuration);
        }
        if publisher == arbiter {
            panic_with_error!(&env, Error::SelfArbiter);
        }

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &publisher,
            env.current_contract_address(),
            &amount,
        );
        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextBond)
            .unwrap_or(0);
        env.storage().instance().set(&DataKey::NextBond, &(id + 1));
        let until = env.ledger().timestamp() + duration;
        let entry = DataKey::Bond(id);
        env.storage().persistent().set(
            &entry,
            &Bond {
                publisher: publisher.clone(),
                arbiter: arbiter.clone(),
                paper_id: paper_id.clone(),
                amount,
                until,
            },
        );
        keep_until(&env, &entry, until + GRACE_SECONDS);
        extend_instance_ttl(&env);

        BondPosted {
            publisher,
            arbiter,
            bond_id: id,
            paper_id,
            amount,
            until,
        }
        .publish(&env);

        id
    }

    /// `arbiter` ejecuta `amount` de la fianza `bond_id` a favor de `to` (un lector afectado, un
    /// fondo de devoluciones): solo dentro del plazo (`BondExpired` después), sin pasar de lo que
    /// queda y nunca hacia sí mismo (`SelfArbiter`). Se puede ejecutar por partes. Sin comisión:
    /// es una compensación. Solo firma `arbiter`. Emite `bond_slashed` y devuelve lo que queda.
    pub fn slash_bond(env: Env, arbiter: Address, bond_id: u32, to: Address, amount: i128) -> i128 {
        arbiter.require_auth();
        let entry = DataKey::Bond(bond_id);
        let mut bond = read_bond(&env, &entry);
        if bond.arbiter != arbiter {
            panic_with_error!(&env, Error::NoBond);
        }
        if env.ledger().timestamp() >= bond.until {
            panic_with_error!(&env, Error::BondExpired);
        }
        if to == arbiter {
            panic_with_error!(&env, Error::SelfArbiter);
        }
        if amount <= 0 || amount > bond.amount {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        bond.amount -= amount;
        env.storage().persistent().set(&entry, &bond);

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &to,
            &amount,
        );
        extend_instance_ttl(&env);

        BondSlashed {
            arbiter,
            to,
            bond_id,
            amount,
            remaining: bond.amount,
        }
        .publish(&env);

        bond.amount
    }

    /// `publisher` recupera lo que queda de su fianza `bond_id`, sin comisión: solo desde que
    /// termina el plazo (`BondActive` antes). Si el árbitro la ejecutó completa, no queda nada
    /// (`NothingToWithdraw`). Solo firma `publisher`. Emite `bond_released` y devuelve lo
    /// recuperado.
    pub fn release_bond(env: Env, publisher: Address, bond_id: u32) -> i128 {
        publisher.require_auth();
        let entry = DataKey::Bond(bond_id);
        let mut bond = read_bond(&env, &entry);
        if bond.publisher != publisher {
            panic_with_error!(&env, Error::NoBond);
        }
        if env.ledger().timestamp() < bond.until {
            panic_with_error!(&env, Error::BondActive);
        }
        if bond.amount <= 0 {
            panic_with_error!(&env, Error::NothingToWithdraw);
        }
        let amount = bond.amount;
        bond.amount = 0;
        env.storage().persistent().set(&entry, &bond);

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &publisher,
            &amount,
        );
        extend_instance_ttl(&env);

        BondReleased {
            publisher,
            bond_id,
            amount,
        }
        .publish(&env);

        amount
    }

    /// `publisher` suma `amount` a su fianza `bond_id` mientras sigue en plazo (`BondExpired`
    /// después): el respaldo crece sin tocar el plazo ni el árbitro. Solo firma `publisher`.
    /// Emite `bond_topped_up` y devuelve el total de la fianza.
    pub fn top_up_bond(env: Env, publisher: Address, bond_id: u32, amount: i128) -> i128 {
        publisher.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let entry = DataKey::Bond(bond_id);
        let mut bond = read_bond(&env, &entry);
        if bond.publisher != publisher {
            panic_with_error!(&env, Error::NoBond);
        }
        if env.ledger().timestamp() >= bond.until {
            panic_with_error!(&env, Error::BondExpired);
        }
        let total = match bond.amount.checked_add(amount) {
            Some(total) => total,
            None => panic_with_error!(&env, Error::AmountOverflow),
        };
        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &publisher,
            env.current_contract_address(),
            &amount,
        );
        bond.amount = total;
        env.storage().persistent().set(&entry, &bond);
        keep_until(&env, &entry, bond.until + GRACE_SECONDS);
        extend_instance_ttl(&env);

        BondToppedUp {
            publisher,
            bond_id,
            amount,
            total,
        }
        .publish(&env);

        total
    }

    /// `arbiter` cierra el caso a favor de la editorial: lo que queda de la fianza `bond_id` vuelve
    /// a su editorial antes del plazo (`BondExpired` después: ahí la recupera ella con
    /// `release_bond`). Solo firma `arbiter`. Emite `bond_cleared` y devuelve lo devuelto.
    pub fn clear_bond(env: Env, arbiter: Address, bond_id: u32) -> i128 {
        arbiter.require_auth();
        let entry = DataKey::Bond(bond_id);
        let mut bond = read_bond(&env, &entry);
        if bond.arbiter != arbiter {
            panic_with_error!(&env, Error::NoBond);
        }
        if env.ledger().timestamp() >= bond.until {
            panic_with_error!(&env, Error::BondExpired);
        }
        if bond.amount <= 0 {
            panic_with_error!(&env, Error::NothingToWithdraw);
        }
        let amount = bond.amount;
        bond.amount = 0;
        env.storage().persistent().set(&entry, &bond);

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &bond.publisher,
            &amount,
        );
        extend_instance_ttl(&env);

        BondCleared {
            arbiter,
            publisher: bond.publisher,
            bond_id,
            amount,
        }
        .publish(&env);

        amount
    }

    /// `publisher` paga `amount` de honorarios al árbitro de su fianza `bond_id`: el árbitro
    /// recibe el neto y la tesorería la comisión, desde la wallet de la editorial y sin tocar la
    /// fianza. Vale también después del plazo. Solo firma `publisher`. Emite `arbiter_paid`.
    pub fn pay_arbiter(env: Env, publisher: Address, bond_id: u32, amount: i128) -> Split {
        publisher.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let entry = DataKey::Bond(bond_id);
        let bond = read_bond(&env, &entry);
        if bond.publisher != publisher {
            panic_with_error!(&env, Error::NoBond);
        }
        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);
        token::Client::new(&env, &config.token).transfer(
            &publisher,
            env.current_contract_address(),
            &amount,
        );
        pay_from_custody(&env, &config, &bond.arbiter, &split);
        extend_instance_ttl(&env);

        ArbiterPaid {
            publisher,
            arbiter: bond.arbiter,
            bond_id,
            amount,
            fee: split.fee,
        }
        .publish(&env);

        split
    }

    /// La fianza `bond_id`, si existe.
    pub fn bond(env: Env, bond_id: u32) -> Option<Bond> {
        env.storage().persistent().get(&DataKey::Bond(bond_id))
    }

    /// `sponsor` apadrina a `reader`: deposita `amount` para sus lecturas con `publisher`. El
    /// dinero pasa de su wallet al contrato y tiene dos salidas: una lectura de ese lector con
    /// esa editorial (`read_sponsored`) o de vuelta a `sponsor` (`reclaim_sponsorship`). Un
    /// depósito nuevo se suma al saldo. No hay operador: el lector lo usa con su propia firma.
    /// Solo firma `sponsor`. Emite `reader_sponsored` y devuelve el saldo.
    pub fn sponsor_reader(
        env: Env,
        sponsor: Address,
        reader: Address,
        publisher: Address,
        amount: i128,
    ) -> i128 {
        sponsor.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let entry = DataKey::Sponsorship(sponsor.clone(), reader.clone(), publisher.clone());
        let before: i128 = env.storage().persistent().get(&entry).unwrap_or(0);
        let balance = match before.checked_add(amount) {
            Some(balance) => balance,
            None => panic_with_error!(&env, Error::AmountOverflow),
        };

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &sponsor,
            env.current_contract_address(),
            &amount,
        );
        env.storage().persistent().set(&entry, &balance);
        keep_max(&env, &entry);
        extend_instance_ttl(&env);

        ReaderSponsored {
            sponsor,
            reader,
            publisher,
            amount,
            balance,
        }
        .publish(&env);

        balance
    }

    /// `reader` paga `amount` por `paper_id` de `publisher` con lo que `sponsor` le dejó para esa
    /// editorial: la editorial recibe el monto menos la comisión y la tesorería, la comisión.
    /// Quien lee no paga nada. Si el saldo no alcanza, falla (`SponsorshipTooLow`). Solo firma
    /// `reader`. Emite `read_taken` y devuelve el reparto.
    pub fn read_sponsored(
        env: Env,
        reader: Address,
        sponsor: Address,
        publisher: Address,
        paper_id: String,
        amount: i128,
    ) -> Split {
        reader.require_auth();
        validate_paper_id(&env, &paper_id);
        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);
        let entry = DataKey::Sponsorship(sponsor, reader.clone(), publisher.clone());
        let balance: i128 = env.storage().persistent().get(&entry).unwrap_or(0);
        if amount > balance {
            panic_with_error!(&env, Error::SponsorshipTooLow);
        }
        if amount == balance {
            env.storage().persistent().remove(&entry);
        } else {
            env.storage().persistent().set(&entry, &(balance - amount));
            keep_max(&env, &entry);
        }

        pay_from_custody(&env, &config, &publisher, &split);
        extend_instance_ttl(&env);

        ReadTaken {
            publisher,
            reader,
            paper_id,
            amount,
            fee: split.fee,
        }
        .publish(&env);

        split
    }

    /// `sponsor` recupera todo lo que dejó para `reader` con `publisher` y no se usó, sin
    /// comisión, cuando quiera. Sin saldo, falla (`NothingToWithdraw`). Solo firma `sponsor`.
    /// Emite `sponsorship_reclaimed` y devuelve lo recuperado.
    pub fn reclaim_sponsorship(
        env: Env,
        sponsor: Address,
        reader: Address,
        publisher: Address,
    ) -> i128 {
        sponsor.require_auth();
        let entry = DataKey::Sponsorship(sponsor.clone(), reader.clone(), publisher.clone());
        let balance: i128 = env.storage().persistent().get(&entry).unwrap_or(0);
        if balance <= 0 {
            panic_with_error!(&env, Error::NothingToWithdraw);
        }
        env.storage().persistent().remove(&entry);

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &sponsor,
            &balance,
        );
        extend_instance_ttl(&env);

        SponsorshipReclaimed {
            sponsor,
            reader,
            publisher,
            amount: balance,
        }
        .publish(&env);

        balance
    }

    /// Lo que `sponsor` tiene depositado para las lecturas de `reader` con `publisher`.
    pub fn sponsorship(env: Env, sponsor: Address, reader: Address, publisher: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Sponsorship(sponsor, reader, publisher))
            .unwrap_or(0)
    }

    /// `first` propone leer `paper_id` de `publisher` a medias con `second` (otra cuenta:
    /// `SamePerson` si es la misma) y pone su mitad de `price`, que pasa de su wallet al
    /// contrato. Con precio impar, quien abre pone el stroop de más. Esa mitad tiene dos salidas:
    /// hacia la editorial cuando `second` ponga la suya (`join_shared`) o de vuelta a `first`
    /// (`cancel_shared`). Solo firma `first`. Emite `shared_opened` y devuelve el id.
    pub fn open_shared(
        env: Env,
        first: Address,
        second: Address,
        publisher: Address,
        paper_id: String,
        price: i128,
    ) -> u32 {
        first.require_auth();
        validate_paper_id(&env, &paper_id);
        // Cada mitad vale al menos un stroop.
        if price < 2 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        if first == second {
            panic_with_error!(&env, Error::SamePerson);
        }
        let half = price - price / 2;

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &first,
            env.current_contract_address(),
            &half,
        );
        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextShared)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::NextShared, &(id + 1));
        let entry = DataKey::Shared(id);
        env.storage().persistent().set(
            &entry,
            &Shared {
                first: first.clone(),
                second: second.clone(),
                publisher: publisher.clone(),
                paper_id: paper_id.clone(),
                price,
                open: true,
            },
        );
        keep_max(&env, &entry);
        extend_instance_ttl(&env);

        SharedOpened {
            first,
            second,
            shared_id: id,
            publisher,
            paper_id,
            price,
            paid: half,
        }
        .publish(&env);

        id
    }

    /// `second` completa la lectura a medias `shared_id`: pone su mitad y, con las dos, la
    /// editorial recibe el precio menos la comisión y la tesorería, la comisión. El artículo
    /// queda pagado para las dos cuentas. Solo quien fue nombrado al abrir (`NoShared`), una sola
    /// vez (`SharedClosed`). Solo firma `second`. Emite `shared_joined` y devuelve el reparto.
    pub fn join_shared(env: Env, second: Address, shared_id: u32) -> Split {
        second.require_auth();
        let entry = DataKey::Shared(shared_id);
        let mut shared = read_shared(&env, &entry);
        if shared.second != second {
            panic_with_error!(&env, Error::NoShared);
        }
        shared.open = false;
        env.storage().persistent().set(&entry, &shared);

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &second,
            env.current_contract_address(),
            &(shared.price / 2),
        );
        let split = compute_split(&env, shared.price, config.fee_bps);
        pay_from_custody(&env, &config, &shared.publisher, &split);
        extend_instance_ttl(&env);

        SharedJoined {
            publisher: shared.publisher,
            first: shared.first,
            second,
            shared_id,
            paper_id: shared.paper_id,
            price: shared.price,
            fee: split.fee,
        }
        .publish(&env);

        split
    }

    /// `first` cancela su lectura a medias `shared_id` mientras la otra cuenta no haya puesto su
    /// mitad: recupera la suya completa, sin comisión. Solo firma `first`. Emite
    /// `shared_cancelled` y devuelve lo recuperado.
    pub fn cancel_shared(env: Env, first: Address, shared_id: u32) -> i128 {
        first.require_auth();
        let entry = DataKey::Shared(shared_id);
        let mut shared = read_shared(&env, &entry);
        if shared.first != first {
            panic_with_error!(&env, Error::NoShared);
        }
        shared.open = false;
        env.storage().persistent().set(&entry, &shared);

        let half = shared.price - shared.price / 2;
        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &first,
            &half,
        );
        extend_instance_ttl(&env);

        SharedCancelled {
            first,
            shared_id,
            amount: half,
        }
        .publish(&env);

        half
    }

    /// La lectura a medias `shared_id`, si existe.
    pub fn shared(env: Env, shared_id: u32) -> Option<Shared> {
        env.storage().persistent().get(&DataKey::Shared(shared_id))
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

fn validate_paper_id(env: &Env, paper_id: &String) {
    if paper_id.is_empty() || paper_id.len() > MAX_PAPER_ID_LEN {
        panic_with_error!(env, Error::InvalidPaperId);
    }
}

fn read_bond(env: &Env, entry: &DataKey) -> Bond {
    match env.storage().persistent().get(entry) {
        Some(bond) => bond,
        None => panic_with_error!(env, Error::NoBond),
    }
}

/// La lectura a medias de `entry`, si existe y sigue abierta.
fn read_shared(env: &Env, entry: &DataKey) -> Shared {
    let shared: Shared = match env.storage().persistent().get(entry) {
        Some(shared) => shared,
        None => panic_with_error!(env, Error::NoShared),
    };
    if !shared.open {
        panic_with_error!(env, Error::SharedClosed);
    }
    shared
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
