#![no_std]

//! Reparto de cada lectura de PaperPay en la misma transacción del pago:
//! la editorial recibe el monto menos la comisión y la tesorería de PaperPay recibe la comisión
//! (`fee_bps`, 200 = 2%). El contrato nunca guarda fondos: mueve el USDC directo de la cuenta
//! del lector a las dos cuentas de destino. Cada pago emite un evento `paid` con el id del
//! artículo, que queda en la red como comprobante de qué se compró.
//!
//! La configuración (token, tesorería y comisión) se fija al desplegar y no cambia: no hay
//! admin ni llave que custodiar. Para cambiarla se despliega otro contrato.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, Env, String,
};

/// 100% en puntos base.
pub const BPS_DENOMINATOR: i128 = 10_000;
/// Comisión máxima aceptada al desplegar (10%): protege a la editorial de un error de captura.
pub const MAX_FEE_BPS: u32 = 1_000;
/// Largo máximo del id de un artículo, en bytes.
pub const MAX_PAPER_ID_LEN: u32 = 64;

// La instancia vive mientras haya pagos: cada `pay` extiende su TTL a ~30 días.
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
    /// El monto es tan grande que el cálculo de la comisión se desborda.
    AmountOverflow = 4,
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
    /// Lo que recibe la editorial.
    pub publisher: i128,
    /// Lo que recibe la tesorería.
    pub fee: i128,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Config,
}

/// Comprobante de una lectura pagada. Topics: `["paid", publisher]`.
#[contractevent(topics = ["paid"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Paid {
    #[topic]
    pub publisher: Address,
    pub paper_id: String,
    pub payer: Address,
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

    /// Cobra una lectura: `amount` sale de `payer`; la editorial recibe `amount - fee` y la
    /// tesorería `fee`. Requiere la firma de `payer` sobre esta llamada (con sus argumentos).
    /// Devuelve el reparto aplicado.
    pub fn pay(
        env: Env,
        payer: Address,
        paper_id: String,
        publisher: Address,
        amount: i128,
    ) -> Split {
        payer.require_auth();

        if paper_id.is_empty() || paper_id.len() > MAX_PAPER_ID_LEN {
            panic_with_error!(&env, Error::InvalidPaperId);
        }
        let config = Self::config(env.clone());
        let split = compute_split(&env, amount, config.fee_bps);

        let usdc = token::Client::new(&env, &config.token);
        if split.publisher > 0 {
            usdc.transfer(&payer, &publisher, &split.publisher);
        }
        if split.fee > 0 {
            usdc.transfer(&payer, &config.treasury, &split.fee);
        }

        env.storage()
            .instance()
            .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND_TO);

        Paid {
            publisher,
            paper_id,
            payer,
            amount,
            fee: split.fee,
        }
        .publish(&env);

        split
    }

    /// Cómo se repartiría `amount`, sin mover fondos.
    pub fn split(env: Env, amount: i128) -> Split {
        let config = Self::config(env.clone());
        compute_split(&env, amount, config.fee_bps)
    }

    pub fn config(env: Env) -> Config {
        env.storage().instance().get(&DataKey::Config).unwrap()
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
