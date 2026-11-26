#![no_std]

//! Reparto de cada lectura de PaperPay en la misma transacción del pago:
//! la editorial recibe el monto menos la comisión y la tesorería de PaperPay recibe la comisión
//! (`fee_bps`, 200 = 2%). Los pagos no pasan por el contrato: mueve el USDC directo de la cuenta
//! de quien paga a las cuentas de destino. Cada artículo pagado emite un evento `paid` con su id
//! y con quién lo puede leer, que queda en la red como comprobante.
//!
//! Seis formas de pagar una lectura, con la misma comisión:
//! - `pay`: un artículo, para quien paga.
//! - `pay_batch`: varios artículos (hasta `MAX_BATCH_ITEMS`) con una sola firma; todo o nada.
//! - `gift`: un artículo que paga una cuenta y lee otra.
//! - `gift_batch`: varios artículos que paga una cuenta y lee otra (una lista de lectura).
//! - `pay_authors`: un artículo cuya parte de editorial se reparte entre coautores.
//! - `pay_ref`: un artículo recomendado por otra cuenta, que se lleva la mitad de la comisión.
//!
//! Accesos con vencimiento, que guardan su fecha en el contrato (sin guardar fondos):
//! - `buy_pass`: un pase de N días al catálogo de una editorial, a precio fijo por día.
//! - `subscribe`: el mismo pase, que se renueva solo: cobra el primer periodo y le da al contrato
//!   una autorización (allowance del token) por el precio de las renovaciones.
//! - `renew_subscription`: cobra una renovación con esa autorización, sin firma de quien lee.
//!   Cualquiera la puede enviar, pero solo cobra el precio pactado y solo cuando al pase le falta
//!   un día o menos. Los fondos siguen en la wallet de quien lee hasta cada cobro.
//! - `sponsor_open`: alguien paga para que un artículo quede abierto para todos N días.
//!
//! Campañas de acceso abierto, la única custodia del contrato:
//! - `open_campaign`: la editorial ofrece liberar un artículo para todos si los aportes llegan a
//!   una meta antes de un plazo.
//! - `pledge`: un aporte hacia la meta. Queda guardado en el contrato hasta que se cumple la meta
//!   (va a la editorial) o vence el plazo sin cumplirla (vuelve a quien aportó). No hay admin: ese
//!   dinero no tiene otra salida.
//!
//! Y dos movimientos que no dan acceso (emiten su propio evento, nunca `paid`):
//! - `tip`: una propina a la editorial por un artículo, con el mismo reparto.
//! - `refund`: la editorial le devuelve dinero a un lector; firma la editorial.
//!
//! La configuración (token, tesorería, comisión y precios por día) se fija al desplegar y no
//! cambia: no hay admin ni llave que custodiar. Para cambiarla se despliega otro contrato.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env, Map, String, Vec,
};

/// 100% en puntos base.
pub const BPS_DENOMINATOR: i128 = 10_000;
/// Comisión máxima aceptada al desplegar (10%): protege a la editorial de un error de captura.
pub const MAX_FEE_BPS: u32 = 1_000;
/// Largo máximo del id de un artículo, en bytes.
pub const MAX_PAPER_ID_LEN: u32 = 64;
/// Artículos por `pay_batch`: acota el costo de una transacción.
pub const MAX_BATCH_ITEMS: u32 = 10;
/// Coautores por `pay_authors`: acota el costo de una transacción.
pub const MAX_AUTHORS: u32 = 10;
/// Parte de la comisión que recibe quien recomienda en `pay_ref` (5 000 = la mitad).
pub const REFERRAL_SHARE_BPS: i128 = 5_000;
/// Días por compra de un pase (y por compra de un artículo abierto).
pub const MAX_PASS_DAYS: u32 = 31;
/// Un acceso acumulado (pase o artículo abierto) no vence más allá de esto, contado desde ahora:
/// así su entrada en la red siempre cabe en el TTL máximo (~180 días en testnet).
pub const MAX_ACCESS_DAYS: u32 = 90;
/// Segundos de un día, en la hora del ledger.
pub const DAY_SECONDS: u64 = 86_400;
/// Renovaciones por suscripción.
pub const MAX_RENEWALS: u32 = 12;
/// Días que cubre una suscripción completa (primer periodo y renovaciones): la autorización del
/// token tiene que caber en el TTL máximo de la red.
pub const MAX_SUBSCRIPTION_DAYS: u32 = 150;
/// Una renovación se puede cobrar cuando al pase le falta esto o menos.
pub const RENEW_WINDOW_SECONDS: u64 = DAY_SECONDS;
/// Plazo máximo de una campaña de acceso abierto, contado desde que se abre.
pub const MAX_CAMPAIGN_DAYS: u32 = 90;
/// Después del plazo, las entradas de una campaña siguen en la red al menos estos días, para que
/// cada aporte de una campaña fallida se pueda devolver.
pub const REFUND_GRACE_DAYS: u32 = 30;

// La instancia vive mientras haya pagos: cada cobro extiende su TTL a ~30 días.
const DAY_IN_LEDGERS: u32 = 17_280;
const INSTANCE_TTL_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;
const INSTANCE_TTL_EXTEND_TO: u32 = 30 * DAY_IN_LEDGERS;
// Un ledger cada ~5 s. Las entradas con vencimiento viven hasta él y una semana más.
const LEDGER_SECONDS: u64 = 5;
const TTL_MARGIN_LEDGERS: u32 = 7 * DAY_IN_LEDGERS;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// `fee_bps` mayor que `MAX_FEE_BPS`.
    FeeTooHigh = 1,
    /// Monto menor o igual a cero.
    InvalidAmount = 2,
    /// Id de artículo vacío o de más de `MAX_PAPER_ID_LEN` bytes.
    InvalidPaperId = 3,
    /// Un monto (o la suma de un paquete) es tan grande que el cálculo se desborda.
    AmountOverflow = 4,
    /// `pay_batch` sin artículos.
    EmptyBatch = 5,
    /// `pay_batch` con más de `MAX_BATCH_ITEMS` artículos.
    BatchTooLarge = 6,
    /// `pay_batch` con el mismo artículo dos veces.
    DuplicatePaper = 7,
    /// `pay_authors` con partes inválidas: lista vacía, una parte en 0 o de más del 100%,
    /// partes que no suman 100% o un autor repetido.
    InvalidShares = 8,
    /// `pay_authors` con más de `MAX_AUTHORS` coautores.
    TooManyAuthors = 9,
    /// `pay_ref` con `referrer` igual a `payer`: sería un descuento, no una recomendación.
    SelfReferral = 10,
    /// Días fuera de rango: 0 o más de `MAX_PASS_DAYS`.
    InvalidDays = 11,
    /// `amount` distinto de días × precio por día de la configuración.
    WrongPrice = 12,
    /// El acceso acumulado vencería después de `MAX_ACCESS_DAYS`, o la suscripción cubre más
    /// de `MAX_SUBSCRIPTION_DAYS`.
    TooLong = 13,
    /// Renovaciones fuera de rango: 0 o más de `MAX_RENEWALS`.
    InvalidRenewals = 14,
    /// `reader` no tiene suscripción (nunca tuvo, la canceló o ya cobró todas las renovaciones).
    NoSubscription = 15,
    /// Al pase le falta más de `RENEW_WINDOW_SECONDS`: todavía no toca renovar.
    RenewTooEarly = 16,
    /// Plazo de campaña que ya pasó o que queda a más de `MAX_CAMPAIGN_DAYS`.
    InvalidDeadline = 17,
    /// No hay campaña con ese id.
    NoCampaign = 18,
    /// La campaña ya no recibe aportes (venció, llegó a la meta o se liberó), o ya se liberó.
    CampaignClosed = 19,
    /// El aporte supera lo que falta para la meta. El monto que se firma no se recorta: si otro
    /// aporte entra antes, este falla sin mover nada y se vuelve a intentar con lo que falte.
    OverGoal = 20,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    /// Contrato del token (el SAC de USDC).
    pub token: Address,
    /// Cuenta que recibe la comisión de PaperPay.
    pub treasury: Address,
    /// Comisión en puntos base (200 = 2%).
    pub fee_bps: u32,
    /// Precio de un día de pase al catálogo de una editorial, en unidades del token.
    pub pass_day_price: i128,
    /// Precio de un día de acceso abierto (para todos) a un artículo, en unidades del token.
    pub open_day_price: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Split {
    /// Lo que reciben las editoriales.
    pub publisher: i128,
    /// Lo que recibe la tesorería.
    pub fee: i128,
}

/// Reparto de `pay_ref`: la editorial recibe lo mismo que en `pay`; la comisión se parte entre
/// la tesorería (`fee`) y quien recomendó (`referral`).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefSplit {
    pub publisher: i128,
    pub fee: i128,
    pub referral: i128,
}

/// Un artículo dentro de `pay_batch`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub paper_id: String,
    pub publisher: Address,
    pub amount: i128,
}

/// Un coautor dentro de `pay_authors`: su cuenta y su parte de lo que le toca a la editorial,
/// en puntos base (2 500 = 25%).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Share {
    pub account: Address,
    pub bps: u32,
}

/// La suscripción de un lector: cada renovación compra `days` días de pase al catálogo de
/// `publisher` por `amount` (días × precio por día). Quedan `renewals_left` renovaciones y la
/// autorización para cobrarlas vence en el ledger `allowance_until`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subscription {
    pub publisher: Address,
    pub days: u32,
    pub amount: i128,
    pub renewals_left: u32,
    pub allowance_until: u32,
}

/// Una campaña de acceso abierto: `publisher` se compromete a liberar `paper_id` para todos si
/// los aportes llegan a `goal` antes de `deadline` (segundos Unix, hora del ledger). `raised` es lo
/// aportado hasta ahora y `released`, si ya se liberó.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Campaign {
    pub publisher: Address,
    pub paper_id: String,
    pub goal: i128,
    pub deadline: u64,
    pub raised: i128,
    pub released: bool,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Config,
    /// Vencimiento (segundos Unix) del pase de `reader` al catálogo de `publisher`.
    Pass(Address, Address),
    /// La suscripción de `reader` (una por lector: la allowance del token es una por cuenta).
    Sub(Address),
    /// Hasta cuándo (segundos Unix) el artículo `paper_id` de `publisher` está abierto para todos.
    Open(Address, String),
    /// Id de la próxima campaña (instance): los ids son secuenciales desde 0.
    NextCampaign,
    /// Una campaña de acceso abierto, por id.
    Campaign(u32),
    /// Lo que aportó `backer` a la campaña `u32` y sigue en garantía.
    Pledge(u32, Address),
}

/// Comprobante de un artículo pagado. Topics: `["paid", publisher, reader]`.
/// `reader` es quien obtiene el acceso: `payer` en `pay` y `pay_batch`, otra cuenta en `gift`.
/// En `pay_authors`, `publisher` es el primer autor de la lista (el de correspondencia).
#[contractevent(topics = ["paid"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Paid {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub paper_id: String,
    pub payer: Address,
    pub amount: i128,
    pub fee: i128,
}

/// Recompensa por recomendar una lectura. Topics: `["referred", referrer, payer]`. Acompaña al
/// `paid` de la misma lectura; `amount` es el precio del artículo y `reward` lo que recibió
/// `referrer` (sale de la comisión).
#[contractevent(topics = ["referred"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Referred {
    #[topic]
    pub referrer: Address,
    #[topic]
    pub payer: Address,
    pub paper_id: String,
    pub publisher: Address,
    pub amount: i128,
    pub reward: i128,
}

/// Comprobante de un pase. Topics: `["pass", publisher, reader]`. `reader` puede leer todo el
/// catálogo de `publisher` hasta `valid_until` (segundos Unix, hora del ledger). `amount` es lo que
/// se cobró por `days` días y `fee` la comisión.
#[contractevent(topics = ["pass"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassBought {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub payer: Address,
    pub days: u32,
    pub amount: i128,
    pub fee: i128,
    pub valid_until: u64,
}

/// Alta de una suscripción. Topics: `["subscribed", publisher, reader]`. Acompaña al `pass` del
/// primer periodo. `renewals` es cuántas renovaciones autorizó `reader`, por `amount` cada una.
#[contractevent(topics = ["subscribed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subscribed {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub days: u32,
    pub amount: i128,
    pub renewals: u32,
    pub allowance_until: u32,
}

/// Una renovación cobrada. Topics: `["renewed", publisher, reader]`. Acompaña al `pass` que
/// extiende el acceso.
#[contractevent(topics = ["renewed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Renewed {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub days: u32,
    pub amount: i128,
    pub fee: i128,
    pub valid_until: u64,
    pub renewals_left: u32,
}

/// Una suscripción cancelada. Topics: `["unsubscribed", publisher, reader]`.
#[contractevent(topics = ["unsubscribed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unsubscribed {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub renewals_left: u32,
}

/// Un artículo abierto para todos por un patrocinio. Topics: `["opened", publisher, sponsor]`.
/// Cualquiera puede leer `paper_id` hasta `open_until` (segundos Unix, hora del ledger).
#[contractevent(topics = ["opened"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Opened {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub sponsor: Address,
    pub paper_id: String,
    pub days: u32,
    pub amount: i128,
    pub fee: i128,
    pub open_until: u64,
}

/// Una campaña nueva. Topics: `["campaign", publisher]`. Es la oferta pública de la editorial:
/// liberar `paper_id` para todos si los aportes llegan a `goal` antes de `deadline`.
#[contractevent(topics = ["campaign"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignOpened {
    #[topic]
    pub publisher: Address,
    pub campaign_id: u32,
    pub paper_id: String,
    pub goal: i128,
    pub deadline: u64,
}

/// Un aporte en garantía. Topics: `["pledged", backer]`. `raised` es lo aportado a la campaña
/// después de este aporte.
#[contractevent(topics = ["pledged"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pledged {
    #[topic]
    pub backer: Address,
    pub campaign_id: u32,
    pub amount: i128,
    pub raised: i128,
}

/// Comprobante de una propina. Topics: `["tipped", publisher, payer]`. No da acceso al artículo.
#[contractevent(topics = ["tipped"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tipped {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub payer: Address,
    pub paper_id: String,
    pub amount: i128,
    pub fee: i128,
}

/// Comprobante de una devolución. Topics: `["refunded", publisher, reader]`.
#[contractevent(topics = ["refunded"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refunded {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub paper_id: String,
    pub amount: i128,
}

#[contract]
pub struct Splitter;

#[contractimpl]
impl Splitter {
    /// `pass_day_price` y `open_day_price` son los precios por día de un pase y de un artículo
    /// abierto: los hace cumplir el contrato, así que quedan fijos como la comisión.
    pub fn __constructor(
        env: Env,
        token: Address,
        treasury: Address,
        fee_bps: u32,
        pass_day_price: i128,
        open_day_price: i128,
    ) {
        if fee_bps > MAX_FEE_BPS {
            panic_with_error!(&env, Error::FeeTooHigh);
        }
        if pass_day_price <= 0 || open_day_price <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let config = Config {
            token,
            treasury,
            fee_bps,
            pass_day_price,
            open_day_price,
        };
        env.storage().instance().set(&DataKey::Config, &config);
    }

    /// Cobra un artículo para quien paga: `amount` sale de `payer`; la editorial recibe
    /// `amount - fee` y la tesorería `fee`. Requiere la firma de `payer` sobre esta llamada
    /// (con sus argumentos). Devuelve el reparto aplicado.
    pub fn pay(
        env: Env,
        payer: Address,
        paper_id: String,
        publisher: Address,
        amount: i128,
    ) -> Split {
        payer.require_auth();
        charge(&env, &payer, &payer, paper_id, publisher, amount)
    }

    /// Cobra varios artículos con una sola firma de `payer`. Cada artículo se reparte igual que
    /// en `pay` y deja su propio evento `paid`; las transferencias se agrupan por editorial.
    /// Si algo falla (un artículo inválido, repetido o sin saldo), no se cobra ninguno.
    /// Devuelve la suma del reparto.
    pub fn pay_batch(env: Env, payer: Address, items: Vec<Item>) -> Split {
        payer.require_auth();
        charge_batch(&env, &payer, &payer, items)
    }

    /// Cobra un artículo a `payer` y deja el acceso a nombre de `reader` (un profesor que paga
    /// la lectura de un estudiante, por ejemplo). Mismo reparto que `pay`. Solo firma `payer`:
    /// recibir un regalo no requiere la firma de `reader`.
    pub fn gift(
        env: Env,
        payer: Address,
        reader: Address,
        paper_id: String,
        publisher: Address,
        amount: i128,
    ) -> Split {
        payer.require_auth();
        charge(&env, &payer, &reader, paper_id, publisher, amount)
    }

    /// Cobra varios artículos a `payer` y deja el acceso de todos a nombre de `reader` (la lista
    /// de lectura de un curso, por ejemplo). Mismas reglas que `pay_batch`: hasta
    /// `MAX_BATCH_ITEMS`, sin repetidos, todo o nada. Solo firma `payer`.
    pub fn gift_batch(env: Env, payer: Address, reader: Address, items: Vec<Item>) -> Split {
        payer.require_auth();
        charge_batch(&env, &payer, &reader, items)
    }

    /// Cobra un artículo a `payer` y reparte la parte de la editorial entre coautores, según la
    /// parte de cada uno (`bps`; entre todos suman 10 000). La tesorería cobra su comisión como en
    /// `pay`. Cada parte se redondea hacia abajo y el residuo va al primer autor: no se pierde
    /// ningún stroop. Emite un `paid` a nombre del primer autor, así que da acceso igual que `pay`.
    /// Devuelve el reparto total.
    pub fn pay_authors(
        env: Env,
        payer: Address,
        paper_id: String,
        authors: Vec<Share>,
        amount: i128,
    ) -> Split {
        payer.require_auth();
        validate_paper_id(&env, &paper_id);
        validate_shares(&env, &authors);
        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);

        let mut parts: Vec<i128> = Vec::new(&env);
        let mut assigned: i128 = 0;
        for share in authors.iter() {
            let part = match split.publisher.checked_mul(share.bps as i128) {
                Some(scaled) => scaled / BPS_DENOMINATOR,
                None => panic_with_error!(&env, Error::AmountOverflow),
            };
            // Cada parte es un piso de `publisher × bps / 10 000`: la suma nunca pasa de `publisher`.
            assigned += part;
            parts.push_back(part);
        }
        parts.set(0, parts.get_unchecked(0) + (split.publisher - assigned));

        let usdc = token::Client::new(&env, &config.token);
        for (share, part) in authors.iter().zip(parts.iter()) {
            if part > 0 {
                usdc.transfer(&payer, &share.account, &part);
            }
        }
        if split.fee > 0 {
            usdc.transfer(&payer, &config.treasury, &split.fee);
        }
        extend_instance_ttl(&env);

        Paid {
            publisher: authors.get_unchecked(0).account,
            reader: payer.clone(),
            paper_id,
            payer,
            amount,
            fee: split.fee,
        }
        .publish(&env);

        split
    }

    /// Cobra un artículo como `pay` y premia a quien lo recomendó: la editorial recibe lo mismo
    /// (98%) y la comisión se parte entre `referrer` (`REFERRAL_SHARE_BPS`, la mitad, redondeada
    /// hacia abajo) y la tesorería (el resto). Emite `paid` (con la comisión total, da acceso a
    /// `payer`) y `referred`. Solo firma `payer`. Devuelve el reparto con la parte del referido.
    pub fn pay_ref(
        env: Env,
        payer: Address,
        paper_id: String,
        publisher: Address,
        amount: i128,
        referrer: Address,
    ) -> RefSplit {
        payer.require_auth();
        if referrer == payer {
            panic_with_error!(&env, Error::SelfReferral);
        }
        validate_paper_id(&env, &paper_id);
        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);
        // `fee` es un cociente entre 10 000: multiplicado por 5 000 no se desborda.
        let reward = split.fee * REFERRAL_SHARE_BPS / BPS_DENOMINATOR;
        let result = RefSplit {
            publisher: split.publisher,
            fee: split.fee - reward,
            referral: reward,
        };

        let usdc = token::Client::new(&env, &config.token);
        if result.publisher > 0 {
            usdc.transfer(&payer, &publisher, &result.publisher);
        }
        if result.referral > 0 {
            usdc.transfer(&payer, &referrer, &result.referral);
        }
        if result.fee > 0 {
            usdc.transfer(&payer, &config.treasury, &result.fee);
        }
        extend_instance_ttl(&env);

        Paid {
            publisher: publisher.clone(),
            reader: payer.clone(),
            paper_id: paper_id.clone(),
            payer: payer.clone(),
            amount,
            fee: split.fee,
        }
        .publish(&env);
        Referred {
            referrer,
            payer,
            paper_id,
            publisher,
            amount,
            reward,
        }
        .publish(&env);

        result
    }

    /// Propina de `payer` a la editorial por un artículo, con el mismo reparto que una lectura.
    /// Emite `tipped`, no `paid`: una propina no da acceso al artículo. Devuelve el reparto.
    pub fn tip(
        env: Env,
        payer: Address,
        paper_id: String,
        publisher: Address,
        amount: i128,
    ) -> Split {
        payer.require_auth();
        validate_paper_id(&env, &paper_id);
        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);
        transfer_split(&env, &config, &payer, &publisher, &split);
        extend_instance_ttl(&env);

        Tipped {
            publisher,
            payer,
            paper_id,
            amount,
            fee: split.fee,
        }
        .publish(&env);

        split
    }

    /// La editorial le devuelve `amount` a `reader` por un artículo. Solo firma la editorial.
    /// El contrato no guarda pagos, así que no comprueba el pago original: eso lo decide quien
    /// devuelve. La comisión ya cobrada no vuelve: la tesorería no firma aquí.
    pub fn refund(env: Env, publisher: Address, reader: Address, paper_id: String, amount: i128) {
        publisher.require_auth();
        validate_paper_id(&env, &paper_id);
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(&publisher, &reader, &amount);
        extend_instance_ttl(&env);

        Refunded {
            publisher,
            reader,
            paper_id,
            amount,
        }
        .publish(&env);
    }

    /// Compra un pase de `days` días (1 a `MAX_PASS_DAYS`) para leer todo el catálogo de
    /// `publisher`. El contrato cobra exactamente `days × pass_day_price` (`amount` tiene que ser
    /// ese monto, así quien firma lo ve) y lo reparte como una lectura, sin guardar fondos. Los
    /// días se suman al pase vigente de `reader`; si ya venció, cuentan desde ahora. Un pase no
    /// vence más allá de `MAX_ACCESS_DAYS` desde ahora. `reader` puede ser otra cuenta (un pase de
    /// regalo): solo firma `payer`. Emite `pass` y devuelve el vencimiento nuevo (segundos Unix,
    /// hora del ledger).
    pub fn buy_pass(
        env: Env,
        payer: Address,
        reader: Address,
        publisher: Address,
        days: u32,
        amount: i128,
    ) -> u64 {
        payer.require_auth();
        let config = read_config(&env);
        check_price(&env, days, config.pass_day_price, amount);
        let split = compute_split(&env, amount, config.fee_bps);
        transfer_split(&env, &config, &payer, &publisher, &split);
        let valid_until = grant_pass(&env, &reader, &publisher, days);
        extend_instance_ttl(&env);

        PassBought {
            publisher,
            reader,
            payer,
            days,
            amount,
            fee: split.fee,
            valid_until,
        }
        .publish(&env);

        valid_until
    }

    /// Hasta cuándo (segundos Unix) puede leer `reader` el catálogo de `publisher`; 0 si nunca
    /// tuvo pase. Un valor menor que la hora del ledger es un pase vencido.
    pub fn pass_until(env: Env, reader: Address, publisher: Address) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::Pass(reader, publisher))
            .unwrap_or(0)
    }

    /// Suscribe a `reader` al catálogo de `publisher`: cobra ya el primer periodo (`days` días de
    /// pase por `amount`, igual que `buy_pass`) y autoriza al contrato a cobrar `renewals`
    /// renovaciones más (1 a `MAX_RENEWALS`) del mismo precio, con `approve` del token. Esa
    /// autorización vence un día después de lo que cubre la suscripción completa, que no puede
    /// pasar de `MAX_SUBSCRIPTION_DAYS`. Los fondos se quedan en la wallet de `reader` hasta cada
    /// renovación. Una suscripción nueva reemplaza a la anterior (y su autorización). Firma
    /// `reader`, una vez. Emite `pass` y `subscribed`; devuelve el vencimiento del pase.
    pub fn subscribe(
        env: Env,
        reader: Address,
        publisher: Address,
        days: u32,
        renewals: u32,
        amount: i128,
    ) -> u64 {
        reader.require_auth();
        let config = read_config(&env);
        check_price(&env, days, config.pass_day_price, amount);
        if renewals == 0 || renewals > MAX_RENEWALS {
            panic_with_error!(&env, Error::InvalidRenewals);
        }
        // Hasta 31 días × 13 periodos: cabe en u32.
        let total_days = days * (renewals + 1);
        if total_days > MAX_SUBSCRIPTION_DAYS {
            panic_with_error!(&env, Error::TooLong);
        }
        let allowance = match amount.checked_mul(renewals as i128) {
            Some(total) => total,
            None => panic_with_error!(&env, Error::AmountOverflow),
        };

        let split = compute_split(&env, amount, config.fee_bps);
        transfer_split(&env, &config, &reader, &publisher, &split);
        let valid_until = grant_pass(&env, &reader, &publisher, days);

        let allowance_until = allowance_until(&env, total_days);
        token::Client::new(&env, &config.token).approve(
            &reader,
            &env.current_contract_address(),
            &allowance,
            &allowance_until,
        );
        let key = DataKey::Sub(reader.clone());
        env.storage().persistent().set(
            &key,
            &Subscription {
                publisher: publisher.clone(),
                days,
                amount,
                renewals_left: renewals,
                allowance_until,
            },
        );
        keep_until(
            &env,
            &key,
            env.ledger().timestamp() + (total_days + 1) as u64 * DAY_SECONDS,
        );
        extend_instance_ttl(&env);

        PassBought {
            publisher: publisher.clone(),
            reader: reader.clone(),
            payer: reader.clone(),
            days,
            amount,
            fee: split.fee,
            valid_until,
        }
        .publish(&env);
        Subscribed {
            publisher,
            reader,
            days,
            amount,
            renewals,
            allowance_until,
        }
        .publish(&env);

        valid_until
    }

    /// Cobra una renovación de la suscripción de `reader` con la autorización que dio al
    /// suscribirse (`transfer_from`), la reparte como una lectura y suma `days` días a su pase.
    /// No pide firmas: la puede enviar cualquiera (la editorial, PaperPay o quien lee), porque
    /// solo cobra el precio pactado y solo cuando al pase le falta `RENEW_WINDOW_SECONDS` o menos;
    /// así nunca se cobra más de un periodo por adelantado. Con la última renovación, la
    /// suscripción termina. Emite `pass` y `renewed`; devuelve el vencimiento nuevo del pase.
    pub fn renew_subscription(env: Env, reader: Address) -> u64 {
        let key = DataKey::Sub(reader.clone());
        let mut sub: Subscription = match env.storage().persistent().get(&key) {
            Some(sub) => sub,
            None => panic_with_error!(&env, Error::NoSubscription),
        };
        let current: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Pass(reader.clone(), sub.publisher.clone()))
            .unwrap_or(0);
        if current > env.ledger().timestamp() + RENEW_WINDOW_SECONDS {
            panic_with_error!(&env, Error::RenewTooEarly);
        }

        let config = read_config(&env);
        let split = compute_split(&env, sub.amount, config.fee_bps);
        let usdc = token::Client::new(&env, &config.token);
        let spender = env.current_contract_address();
        if split.publisher > 0 {
            usdc.transfer_from(&spender, &reader, &sub.publisher, &split.publisher);
        }
        if split.fee > 0 {
            usdc.transfer_from(&spender, &reader, &config.treasury, &split.fee);
        }
        let valid_until = grant_pass(&env, &reader, &sub.publisher, sub.days);

        sub.renewals_left -= 1;
        if sub.renewals_left == 0 {
            env.storage().persistent().remove(&key);
        } else {
            env.storage().persistent().set(&key, &sub);
        }
        extend_instance_ttl(&env);

        PassBought {
            publisher: sub.publisher.clone(),
            reader: reader.clone(),
            payer: reader.clone(),
            days: sub.days,
            amount: sub.amount,
            fee: split.fee,
            valid_until,
        }
        .publish(&env);
        Renewed {
            publisher: sub.publisher,
            reader,
            days: sub.days,
            amount: sub.amount,
            fee: split.fee,
            valid_until,
            renewals_left: sub.renewals_left,
        }
        .publish(&env);

        valid_until
    }

    /// Cancela la suscripción de `reader`: borra las renovaciones pendientes y deja en cero la
    /// autorización del token. El pase ya pagado sigue hasta su vencimiento. Firma `reader`.
    /// Emite `unsubscribed`.
    pub fn cancel_subscription(env: Env, reader: Address) {
        reader.require_auth();
        let key = DataKey::Sub(reader.clone());
        let sub: Subscription = match env.storage().persistent().get(&key) {
            Some(sub) => sub,
            None => panic_with_error!(&env, Error::NoSubscription),
        };
        env.storage().persistent().remove(&key);
        let config = read_config(&env);
        // Monto 0 con vencimiento 0: el token lo acepta y la llamada firmada no depende del
        // ledger en que se envía.
        token::Client::new(&env, &config.token).approve(
            &reader,
            &env.current_contract_address(),
            &0,
            &0,
        );
        extend_instance_ttl(&env);

        Unsubscribed {
            publisher: sub.publisher,
            reader,
            renewals_left: sub.renewals_left,
        }
        .publish(&env);
    }

    /// La suscripción vigente de `reader`, si tiene.
    pub fn subscription(env: Env, reader: Address) -> Option<Subscription> {
        env.storage().persistent().get(&DataKey::Sub(reader))
    }

    /// `sponsor` paga para que `paper_id` de `publisher` quede abierto para todos `days` días
    /// (1 a `MAX_PASS_DAYS`): un laboratorio, una fundación o la misma editorial patrocina el
    /// acceso abierto. Cobra exactamente `days × open_day_price` y lo reparte como una lectura,
    /// sin guardar fondos. Los días se suman a la apertura vigente; si ya venció, cuentan desde
    /// ahora, y no pasa de `MAX_ACCESS_DAYS` desde ahora. Solo firma `sponsor`. Emite `opened` (no
    /// `paid`: no es la lectura de una cuenta) y devuelve hasta cuándo queda abierto.
    pub fn sponsor_open(
        env: Env,
        sponsor: Address,
        paper_id: String,
        publisher: Address,
        days: u32,
        amount: i128,
    ) -> u64 {
        sponsor.require_auth();
        validate_paper_id(&env, &paper_id);
        let config = read_config(&env);
        check_price(&env, days, config.open_day_price, amount);
        let split = compute_split(&env, amount, config.fee_bps);
        transfer_split(&env, &config, &sponsor, &publisher, &split);
        let open_until = extend_access(
            &env,
            &DataKey::Open(publisher.clone(), paper_id.clone()),
            days,
        );
        extend_instance_ttl(&env);

        Opened {
            publisher,
            sponsor,
            paper_id,
            days,
            amount,
            fee: split.fee,
            open_until,
        }
        .publish(&env);

        open_until
    }

    /// Hasta cuándo (segundos Unix) está abierto para todos `paper_id` de `publisher`; 0 si nunca
    /// se abrió.
    pub fn open_until(env: Env, publisher: Address, paper_id: String) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::Open(publisher, paper_id))
            .unwrap_or(0)
    }

    /// `publisher` ofrece liberar `paper_id` para todos si los aportes llegan a `goal` antes de
    /// `deadline` (segundos Unix, hora del ledger; como mucho `MAX_CAMPAIGN_DAYS` desde ahora). Su
    /// firma es la oferta pública: nadie más puede abrir una campaña por su artículo. Los ids son
    /// secuenciales, así que nadie puede ocupar el id de la campaña de otra editorial. No mueve
    /// fondos. Emite `campaign` y devuelve el id.
    pub fn open_campaign(
        env: Env,
        publisher: Address,
        paper_id: String,
        goal: i128,
        deadline: u64,
    ) -> u32 {
        publisher.require_auth();
        validate_paper_id(&env, &paper_id);
        if goal <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let now = env.ledger().timestamp();
        if deadline <= now || deadline - now > MAX_CAMPAIGN_DAYS as u64 * DAY_SECONDS {
            panic_with_error!(&env, Error::InvalidDeadline);
        }

        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextCampaign)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::NextCampaign, &(id + 1));
        let key = DataKey::Campaign(id);
        env.storage().persistent().set(
            &key,
            &Campaign {
                publisher: publisher.clone(),
                paper_id: paper_id.clone(),
                goal,
                deadline,
                raised: 0,
                released: false,
            },
        );
        keep_campaign(&env, &key, deadline);
        extend_instance_ttl(&env);

        CampaignOpened {
            publisher,
            campaign_id: id,
            paper_id,
            goal,
            deadline,
        }
        .publish(&env);

        id
    }

    /// `backer` aporta `amount` a la campaña `campaign_id`. El aporte pasa de su wallet al
    /// contrato y queda en garantía: si se llega a la meta, va a la editorial (`release_pledges`);
    /// si vence el plazo sin llegar, vuelve completo a `backer` (`refund_pledge`). Solo mientras la
    /// campaña está abierta (antes del plazo, sin la meta cumplida y sin liberar) y sin pasar de
    /// lo que falta (`OverGoal`): el monto firmado es el que se cobra, nunca se recorta. Solo firma
    /// `backer`. Emite `pledged` y devuelve lo aportado a la campaña hasta ahora.
    pub fn pledge(env: Env, backer: Address, campaign_id: u32, amount: i128) -> i128 {
        backer.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let key = DataKey::Campaign(campaign_id);
        let mut campaign = read_campaign(&env, campaign_id);
        if campaign.released
            || env.ledger().timestamp() >= campaign.deadline
            || campaign.raised >= campaign.goal
        {
            panic_with_error!(&env, Error::CampaignClosed);
        }
        if amount > campaign.goal - campaign.raised {
            panic_with_error!(&env, Error::OverGoal);
        }

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &backer,
            env.current_contract_address(),
            &amount,
        );
        // Ni el total ni el aporte de una cuenta pasan de la meta: no se desbordan.
        campaign.raised += amount;
        let pledge_key = DataKey::Pledge(campaign_id, backer.clone());
        let pledged: i128 = env.storage().persistent().get(&pledge_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&pledge_key, &(pledged + amount));
        env.storage().persistent().set(&key, &campaign);
        keep_campaign(&env, &key, campaign.deadline);
        keep_campaign(&env, &pledge_key, campaign.deadline);
        extend_instance_ttl(&env);

        Pledged {
            backer,
            campaign_id,
            amount,
            raised: campaign.raised,
        }
        .publish(&env);

        campaign.raised
    }

    /// Lo que aportó `backer` a la campaña `campaign_id` y sigue en garantía.
    pub fn pledge_of(env: Env, campaign_id: u32, backer: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Pledge(campaign_id, backer))
            .unwrap_or(0)
    }

    /// La campaña `campaign_id`, si existe.
    pub fn campaign(env: Env, campaign_id: u32) -> Option<Campaign> {
        env.storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
    }

    /// Cómo se repartiría `amount`, sin mover fondos.
    pub fn split(env: Env, amount: i128) -> Split {
        let config = read_config(&env);
        compute_split(&env, amount, config.fee_bps)
    }

    pub fn config(env: Env) -> Config {
        read_config(&env)
    }
}

/// Un cobro individual (`pay` y `gift`): reparte, extiende el TTL y emite el comprobante.
fn charge(
    env: &Env,
    payer: &Address,
    reader: &Address,
    paper_id: String,
    publisher: Address,
    amount: i128,
) -> Split {
    validate_paper_id(env, &paper_id);
    let config = read_config(env);
    let split = compute_split(env, amount, config.fee_bps);

    transfer_split(env, &config, payer, &publisher, &split);
    extend_instance_ttl(env);

    Paid {
        publisher,
        reader: reader.clone(),
        paper_id,
        payer: payer.clone(),
        amount,
        fee: split.fee,
    }
    .publish(env);

    split
}

/// Varios cobros con una firma (`pay_batch` y `gift_batch`): valida todo antes de mover fondos,
/// agrupa las transferencias por editorial y emite un `paid` por artículo, a nombre de `reader`.
fn charge_batch(env: &Env, payer: &Address, reader: &Address, items: Vec<Item>) -> Split {
    if items.is_empty() {
        panic_with_error!(env, Error::EmptyBatch);
    }
    if items.len() > MAX_BATCH_ITEMS {
        panic_with_error!(env, Error::BatchTooLarge);
    }

    let config = read_config(env);
    let mut seen: Map<String, bool> = Map::new(env);
    let mut per_publisher: Map<Address, i128> = Map::new(env);
    let mut fees: Vec<i128> = Vec::new(env);
    let mut total = Split {
        publisher: 0,
        fee: 0,
    };

    for item in items.iter() {
        validate_paper_id(env, &item.paper_id);
        if seen.contains_key(item.paper_id.clone()) {
            panic_with_error!(env, Error::DuplicatePaper);
        }
        seen.set(item.paper_id.clone(), true);

        let split = compute_split(env, item.amount, config.fee_bps);
        let owed = per_publisher.get(item.publisher.clone()).unwrap_or(0);
        per_publisher.set(item.publisher.clone(), add(env, owed, split.publisher));
        total.publisher = add(env, total.publisher, split.publisher);
        total.fee = add(env, total.fee, split.fee);
        fees.push_back(split.fee);
    }

    let usdc = token::Client::new(env, &config.token);
    for (publisher, owed) in per_publisher.iter() {
        if owed > 0 {
            usdc.transfer(payer, &publisher, &owed);
        }
    }
    if total.fee > 0 {
        usdc.transfer(payer, &config.treasury, &total.fee);
    }
    extend_instance_ttl(env);

    for (item, fee) in items.iter().zip(fees.iter()) {
        Paid {
            publisher: item.publisher,
            reader: reader.clone(),
            paper_id: item.paper_id,
            payer: payer.clone(),
            amount: item.amount,
            fee,
        }
        .publish(env);
    }

    total
}

/// Mueve un reparto de `payer` a la editorial y a la tesorería (omite montos en cero).
fn transfer_split(env: &Env, config: &Config, payer: &Address, publisher: &Address, split: &Split) {
    let usdc = token::Client::new(env, &config.token);
    if split.publisher > 0 {
        usdc.transfer(payer, publisher, &split.publisher);
    }
    if split.fee > 0 {
        usdc.transfer(payer, &config.treasury, &split.fee);
    }
}

/// Suma `days` días al pase de `reader` con `publisher` y devuelve el vencimiento nuevo.
fn grant_pass(env: &Env, reader: &Address, publisher: &Address, days: u32) -> u64 {
    extend_access(env, &DataKey::Pass(reader.clone(), publisher.clone()), days)
}

/// Suma `days` días a un acceso con vencimiento: desde el vencimiento vigente si sigue en pie, o
/// desde ahora si ya venció. Lo guarda con un TTL que lo cubre y devuelve el vencimiento nuevo.
fn extend_access(env: &Env, key: &DataKey, days: u32) -> u64 {
    let now = env.ledger().timestamp();
    let current: u64 = env.storage().persistent().get(key).unwrap_or(0);
    let until = current.max(now) + days as u64 * DAY_SECONDS;
    if until - now > MAX_ACCESS_DAYS as u64 * DAY_SECONDS {
        panic_with_error!(env, Error::TooLong);
    }
    env.storage().persistent().set(key, &until);
    keep_until(env, key, until);
    until
}

/// Extiende el TTL de una entrada persistente para que siga en la red hasta `until` (segundos
/// Unix) y `TTL_MARGIN_LEDGERS` más, sin pasar del máximo de la red.
fn keep_until(env: &Env, key: &DataKey, until: u64) {
    let secs = until.saturating_sub(env.ledger().timestamp());
    let ledgers = (secs / LEDGER_SECONDS) as u32 + TTL_MARGIN_LEDGERS;
    let ttl = ledgers.min(env.storage().max_ttl());
    env.storage().persistent().extend_ttl(key, ttl, ttl);
}

fn read_campaign(env: &Env, campaign_id: u32) -> Campaign {
    match env
        .storage()
        .persistent()
        .get(&DataKey::Campaign(campaign_id))
    {
        Some(campaign) => campaign,
        None => panic_with_error!(env, Error::NoCampaign),
    }
}

/// Las entradas de una campaña viven hasta su plazo y `REFUND_GRACE_DAYS` más (y el margen de
/// `keep_until`): así cada aporte de una campaña fallida se puede devolver.
fn keep_campaign(env: &Env, key: &DataKey, deadline: u64) {
    keep_until(env, key, deadline + REFUND_GRACE_DAYS as u64 * DAY_SECONDS);
}

/// Ledger en que vence la autorización de una suscripción que cubre `total_days`: al menos un día
/// después de la última renovación. Se redondea al día de ledgers porque es un argumento de
/// `approve`, que quien lee firma: si dependiera del ledger exacto, el valor simulado y el
/// ejecutado no coincidirían y la red rechazaría la firma. Solo cambia si entre simular y enviar
/// la red cruza un múltiplo de `DAY_IN_LEDGERS` (una vez al día); entonces basta reintentar.
fn allowance_until(env: &Env, total_days: u32) -> u32 {
    (env.ledger().sequence() / DAY_IN_LEDGERS + total_days + 2) * DAY_IN_LEDGERS
}

/// Días entre 1 y `MAX_PASS_DAYS`, y `amount` exactamente `days × day_price`.
fn check_price(env: &Env, days: u32, day_price: i128, amount: i128) {
    if days == 0 || days > MAX_PASS_DAYS {
        panic_with_error!(env, Error::InvalidDays);
    }
    let expected = match day_price.checked_mul(days as i128) {
        Some(total) => total,
        None => panic_with_error!(env, Error::AmountOverflow),
    };
    if amount != expected {
        panic_with_error!(env, Error::WrongPrice);
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

fn validate_paper_id(env: &Env, paper_id: &String) {
    if paper_id.is_empty() || paper_id.len() > MAX_PAPER_ID_LEN {
        panic_with_error!(env, Error::InvalidPaperId);
    }
}

/// Partes de `pay_authors`: de 1 a `MAX_AUTHORS` autores distintos, cada parte entre 1 y 10 000
/// puntos base, y entre todas exactamente 10 000 (100%).
fn validate_shares(env: &Env, authors: &Vec<Share>) {
    if authors.len() > MAX_AUTHORS {
        panic_with_error!(env, Error::TooManyAuthors);
    }
    if authors.is_empty() {
        panic_with_error!(env, Error::InvalidShares);
    }
    let mut seen: Map<Address, bool> = Map::new(env);
    let mut total: u32 = 0;
    for share in authors.iter() {
        if share.bps == 0 || share.bps as i128 > BPS_DENOMINATOR {
            panic_with_error!(env, Error::InvalidShares);
        }
        if seen.contains_key(share.account.clone()) {
            panic_with_error!(env, Error::InvalidShares);
        }
        seen.set(share.account, true);
        // Hasta `MAX_AUTHORS` partes de 10 000 como máximo: no se desborda.
        total += share.bps;
    }
    if total as i128 != BPS_DENOMINATOR {
        panic_with_error!(env, Error::InvalidShares);
    }
}

fn add(env: &Env, a: i128, b: i128) -> i128 {
    match a.checked_add(b) {
        Some(sum) => sum,
        None => panic_with_error!(env, Error::AmountOverflow),
    }
}

/// La comisión se redondea hacia abajo: cualquier fracción de stroop queda para la editorial.
fn compute_split(env: &Env, amount: i128, fee_bps: u32) -> Split {
    if amount <= 0 {
        panic_with_error!(env, Error::InvalidAmount);
    }
    let fee = match amount.checked_mul(fee_bps as i128) {
        Some(scaled) => scaled / BPS_DENOMINATOR,
        None => panic_with_error!(env, Error::AmountOverflow),
    };
    Split {
        publisher: amount - fee,
        fee,
    }
}

#[cfg(test)]
mod test;
