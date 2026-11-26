#![cfg(test)]
extern crate std;

use super::{Boveda, BovedaClient, Claimed, Config, Deposited, Error};
use soroban_sdk::{
    testutils::{
        storage::Instance as _, Address as _, AuthorizedFunction, AuthorizedInvocation,
        Events as _, Ledger as _,
    },
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env, Event, IntoVal, String, Symbol,
};

/// 5555 XLM con 7 decimales: lo que pide el reto.
const PRIZE: i128 = 55_550_000_000;
/// SHA-256 de "Ship It" (UTF-8). El script de lanzamiento usa este mismo valor
/// (`printf 'Ship It' | sha256sum`).
const SHIP_IT_SHA256: &str = "7121f6ba9548cb8cf29a1d8dafcae8cefd0f5b97dbb5af2532eb6ffcb799e66e";

struct Setup<'a> {
    env: Env,
    xlm: TokenClient<'a>,
    boveda: BovedaClient<'a>,
    funder: Address,
}

fn hex32(env: &Env, hex: &str) -> BytesN<32> {
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
    }
    BytesN::from_array(env, &out)
}

fn setup() -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();
    // Límites de TTL de testnet (protocolo 30).
    env.ledger().with_mut(|li| {
        li.sequence_number = 6_000_000;
        li.max_entry_ttl = 3_110_400;
        li.min_persistent_entry_ttl = 120_960;
        li.min_temp_entry_ttl = 720;
    });
    let sac = env.register_stellar_asset_contract_v2(Address::generate(&env));
    let funder = Address::generate(&env);
    StellarAssetClient::new(&env, &sac.address()).mint(&funder, &(3 * PRIZE));
    let id = env.register(Boveda, (sac.address(), hex32(&env, SHIP_IT_SHA256)));
    Setup {
        xlm: TokenClient::new(&env, &sac.address()),
        boveda: BovedaClient::new(&env, &id),
        funder,
        env,
    }
}

impl Setup<'_> {
    fn code(&self, text: &str) -> String {
        String::from_str(&self.env, text)
    }

    fn funded(&self) {
        self.boveda.deposit(&self.funder, &PRIZE);
    }

    fn events(&self) -> soroban_sdk::testutils::ContractEvents {
        self.env
            .events()
            .all()
            .filter_by_contract(&self.boveda.address)
    }
}

#[test]
fn ship_it_hash_matches_the_constant() {
    let env = Env::default();
    let bytes = String::from_str(&env, "Ship It").to_bytes();
    assert_eq!(
        env.crypto().sha256(&bytes).to_bytes(),
        hex32(&env, SHIP_IT_SHA256)
    );
}

#[test]
fn constructor_keeps_token_and_hash_only() {
    let s = setup();
    assert_eq!(
        s.boveda.config(),
        Config {
            token: s.xlm.address.clone(),
            code_hash: hex32(&s.env, SHIP_IT_SHA256),
        }
    );
    assert_eq!(s.boveda.balance(), 0);
}

#[test]
fn deposit_moves_funds_with_the_funder_signature() {
    let s = setup();
    let balance = s.boveda.deposit(&s.funder, &PRIZE);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.funder.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.boveda.address.clone(),
                    Symbol::new(&s.env, "deposit"),
                    (s.funder.clone(), PRIZE).into_val(&s.env),
                )),
                sub_invocations: std::vec![AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        s.xlm.address.clone(),
                        Symbol::new(&s.env, "transfer"),
                        (s.funder.clone(), s.boveda.address.clone(), PRIZE).into_val(&s.env),
                    )),
                    sub_invocations: std::vec![],
                }],
            }
        )]
    );
    let expected = Deposited {
        from: s.funder.clone(),
        amount: PRIZE,
        balance: PRIZE,
    };
    assert_eq!(s.events(), [expected.to_xdr(&s.env, &s.boveda.address)]);
    assert_eq!(balance, PRIZE);
    assert_eq!(s.boveda.balance(), PRIZE);
    assert_eq!(s.xlm.balance(&s.funder), 2 * PRIZE);
}

#[test]
fn deposit_rejects_zero_and_negative() {
    let s = setup();
    for amount in [0, -1] {
        assert_eq!(
            s.boveda.try_deposit(&s.funder, &amount),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    assert_eq!(s.boveda.balance(), 0);
}

#[test]
fn claim_with_ship_it_takes_everything() {
    let s = setup();
    s.funded();
    let judge = Address::generate(&s.env);
    let amount = s.boveda.claim(&judge, &s.code("Ship It"));
    assert_eq!(
        s.env.auths(),
        std::vec![(
            judge.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.boveda.address.clone(),
                    Symbol::new(&s.env, "claim"),
                    (judge.clone(), s.code("Ship It")).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
    let expected = Claimed {
        claimer: judge.clone(),
        amount: PRIZE,
    };
    assert_eq!(s.events(), [expected.to_xdr(&s.env, &s.boveda.address)]);
    assert_eq!(amount, PRIZE);
    assert_eq!(s.xlm.balance(&judge), PRIZE);
    assert_eq!(s.boveda.balance(), 0);
}

#[test]
fn claim_rejects_any_other_code() {
    let s = setup();
    s.funded();
    let judge = Address::generate(&s.env);
    // El contrato compara exacto: la pantalla es la que normaliza mayúsculas y espacios.
    for code in ["ship it", "Ship it", "Ship It ", "«Ship It»", "ShipIt", ""] {
        assert_eq!(
            s.boveda.try_claim(&judge, &s.code(code)),
            Err(Ok(Error::WrongCode.into())),
            "{code:?}"
        );
    }
    assert_eq!(s.boveda.balance(), PRIZE);
    assert_eq!(s.xlm.balance(&judge), 0);
}

#[test]
fn claim_on_empty_vault_fails() {
    let s = setup();
    let judge = Address::generate(&s.env);
    assert_eq!(
        s.boveda.try_claim(&judge, &s.code("Ship It")),
        Err(Ok(Error::Empty.into()))
    );
}

#[test]
fn second_claim_finds_it_empty_until_refilled() {
    let s = setup();
    s.funded();
    let first = Address::generate(&s.env);
    let judge = Address::generate(&s.env);
    s.boveda.claim(&first, &s.code("Ship It"));
    assert_eq!(
        s.boveda.try_claim(&judge, &s.code("Ship It")),
        Err(Ok(Error::Empty.into()))
    );
    // Relleno: el siguiente vuelve a llevarse los 5555.
    s.funded();
    assert_eq!(s.boveda.claim(&judge, &s.code("Ship It")), PRIZE);
    assert_eq!(s.xlm.balance(&first), PRIZE);
    assert_eq!(s.xlm.balance(&judge), PRIZE);
    assert_eq!(s.boveda.balance(), 0);
}

#[test]
fn claim_takes_extra_deposits_too() {
    let s = setup();
    s.funded();
    // deposit devuelve el saldo, no el monto.
    assert_eq!(s.boveda.deposit(&s.funder, &1), PRIZE + 1);
    let judge = Address::generate(&s.env);
    assert_eq!(s.boveda.claim(&judge, &s.code("Ship It")), PRIZE + 1);
    assert_eq!(s.boveda.balance(), 0);
}

#[test]
fn every_use_keeps_the_vault_alive_for_the_max() {
    let s = setup();
    let max = 3_110_400 - 1;
    let ttl = || {
        s.env
            .as_contract(&s.boveda.address, || s.env.storage().instance().get_ttl())
    };
    assert_eq!(ttl(), max);
    s.env.ledger().with_mut(|li| li.sequence_number += 100_000);
    s.funded();
    assert_eq!(ttl(), max);
    s.env.ledger().with_mut(|li| li.sequence_number += 100_000);
    s.boveda
        .claim(&Address::generate(&s.env), &s.code("Ship It"));
    assert_eq!(ttl(), max);
}
