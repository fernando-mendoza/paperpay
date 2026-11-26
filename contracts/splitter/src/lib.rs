#![no_std]

//! Reparto de cada lectura de PaperPay en la misma transacción del pago:
//! la editorial recibe el monto menos la comisión y la tesorería de PaperPay recibe la comisión
//! (`fee_bps`, 200 = 2%). El contrato nunca guarda fondos: mueve el USDC directo de la cuenta
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

// La instancia vive mientras haya pagos: cada cobro extiende su TTL a ~30 días.
const DAY_IN_LEDGERS: u32 = 17_280;
const INSTANCE_TTL_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;
const INSTANCE_TTL_EXTEND_TO: u32 = 30 * DAY_IN_LEDGERS;

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

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Config,
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
