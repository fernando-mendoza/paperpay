#![no_std]

//! Reparto de cada lectura de PaperPay en la misma transacción del pago:
//! la editorial recibe el monto menos la comisión y la tesorería de PaperPay recibe la comisión
//! (`fee_bps`, 200 = 2%). El contrato nunca guarda fondos: mueve el USDC directo de la cuenta
//! de quien paga a las cuentas de destino. Cada artículo pagado emite un evento `paid` con su id
//! y con quién lo puede leer, que queda en la red como comprobante.
//!
//! Cuatro formas de pagar una lectura, con la misma regla de reparto:
//! - `pay`: un artículo, para quien paga.
//! - `pay_batch`: varios artículos (hasta `MAX_BATCH_ITEMS`) con una sola firma; todo o nada.
//! - `gift`: un artículo que paga una cuenta y lee otra.
//! - `gift_batch`: varios artículos que paga una cuenta y lee otra (una lista de lectura).
//!
//! Y un movimiento que no da acceso (emite su propio evento, nunca `paid`):
//! - `tip`: una propina a la editorial por un artículo, con el mismo reparto.
//!
//! La configuración (token, tesorería y comisión) se fija al desplegar y no cambia: no hay
//! admin ni llave que custodiar. Para cambiarla se despliega otro contrato.

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
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Split {
    /// Lo que reciben las editoriales.
    pub publisher: i128,
    /// Lo que recibe la tesorería.
    pub fee: i128,
}

/// Un artículo dentro de `pay_batch`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub paper_id: String,
    pub publisher: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Config,
}

/// Comprobante de un artículo pagado. Topics: `["paid", publisher, reader]`.
/// `reader` es quien obtiene el acceso: `payer` en `pay` y `pay_batch`, otra cuenta en `gift`.
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

#[contract]
pub struct Splitter;

#[contractimpl]
impl Splitter {
    pub fn __constructor(env: Env, token: Address, treasury: Address, fee_bps: u32) {
        if fee_bps > MAX_FEE_BPS {
            panic_with_error!(&env, Error::FeeTooHigh);
        }
        let config = Config {
            token,
            treasury,
            fee_bps,
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
