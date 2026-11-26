#![no_std]

//! Bóveda de PaperPay para el reto exprés de Ship It: guarda XLM y solo los suelta a quien
//! presenta el código.
//!
//! - `deposit`: cualquiera llena la bóveda con XLM (el token se fija al desplegar).
//! - `claim`: quien presenta el código correcto se lleva todo lo que hay, con su firma. El
//!   contrato no guarda el código, solo su SHA-256: compara el hash de lo que le mandan.
//!
//! No hay admin ni retiro: los fondos solo salen con el código. El código viaja en claro en la
//! transacción de `claim` (así la wallet lo muestra al firmar), así que la bóveda es un reto, no
//! una caja fuerte: cualquiera que lo sepa puede vaciarla.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, BytesN, Env, String,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Depósito menor o igual a cero.
    InvalidAmount = 1,
    /// El SHA-256 del código no coincide con el de la bóveda.
    WrongCode = 2,
    /// La bóveda no tiene saldo.
    Empty = 3,
}

/// Lo que se fija al desplegar y no cambia.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    /// El token que guarda la bóveda (en testnet, el SAC de XLM).
    pub token: Address,
    /// SHA-256 del código, en bytes UTF-8.
    pub code_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Config,
}

/// Un depósito. Topics: `["deposited", from]`.
#[contractevent(topics = ["deposited"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deposited {
    #[topic]
    pub from: Address,
    pub amount: i128,
    /// Saldo de la bóveda después del depósito.
    pub balance: i128,
}

/// La bóveda abierta con el código. Topics: `["claimed", claimer]`.
#[contractevent(topics = ["claimed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claimed {
    #[topic]
    pub claimer: Address,
    pub amount: i128,
}

#[contract]
pub struct Boveda;

#[contractimpl]
impl Boveda {
    pub fn __constructor(env: Env, token: Address, code_hash: BytesN<32>) {
        env.storage()
            .instance()
            .set(&DataKey::Config, &Config { token, code_hash });
        keep_alive(&env);
    }

    /// Mete `amount` del token en la bóveda, con la firma de `from`. Devuelve el saldo nuevo.
    pub fn deposit(env: Env, from: Address, amount: i128) -> i128 {
        from.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let token = token::Client::new(&env, &config(&env).token);
        let vault = env.current_contract_address();
        token.transfer(&from, &vault, &amount);
        let balance = token.balance(&vault);
        Deposited {
            from,
            amount,
            balance,
        }
        .publish(&env);
        keep_alive(&env);
        balance
    }

    /// Con la firma de `claimer` y el código correcto, le transfiere todo el saldo. Devuelve el
    /// monto. `WrongCode` si el código no es; `Empty` si ya no hay nada.
    pub fn claim(env: Env, claimer: Address, code: String) -> i128 {
        claimer.require_auth();
        let config = config(&env);
        if env.crypto().sha256(&code.to_bytes()).to_bytes() != config.code_hash {
            panic_with_error!(&env, Error::WrongCode);
        }
        let token = token::Client::new(&env, &config.token);
        let vault = env.current_contract_address();
        let amount = token.balance(&vault);
        if amount <= 0 {
            panic_with_error!(&env, Error::Empty);
        }
        token.transfer(&vault, &claimer, &amount);
        Claimed { claimer, amount }.publish(&env);
        keep_alive(&env);
        amount
    }

    /// Saldo de la bóveda.
    pub fn balance(env: Env) -> i128 {
        token::Client::new(&env, &config(&env).token).balance(&env.current_contract_address())
    }

    pub fn config(env: Env) -> Config {
        config(&env)
    }
}

fn config(env: &Env) -> Config {
    env.storage().instance().get(&DataKey::Config).unwrap()
}

/// La bóveda (instancia y código) vive el máximo que permite la red desde cada uso.
fn keep_alive(env: &Env) {
    let max = env.storage().max_ttl();
    env.storage().instance().extend_ttl(max, max);
}

mod test;
