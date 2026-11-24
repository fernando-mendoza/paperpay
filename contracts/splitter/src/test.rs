#![cfg(test)]
extern crate std;

use super::{Config, Error, Paid, Split, Splitter, SplitterClient, MAX_FEE_BPS};
use soroban_sdk::{
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation, Events as _},
    token::{StellarAssetClient, TokenClient},
    Address, Env, Event, IntoVal, String, Symbol,
};

/// $0.50 USDC con 7 decimales.
const PRICE: i128 = 5_000_000;
const FEE_BPS: u32 = 200;

struct Setup<'a> {
    env: Env,
    splitter: SplitterClient<'a>,
    usdc: TokenClient<'a>,
    payer: Address,
    publisher: Address,
    treasury: Address,
}

fn setup() -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let issuer = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(issuer).address();
    let payer = Address::generate(&env);
    let publisher = Address::generate(&env);
    let treasury = Address::generate(&env);
    StellarAssetClient::new(&env, &usdc).mint(&payer, &(10 * PRICE));

    let id = env.register(Splitter, (usdc.clone(), treasury.clone(), FEE_BPS));
    Setup {
        splitter: SplitterClient::new(&env, &id),
        usdc: TokenClient::new(&env, &usdc),
        env,
        payer,
        publisher,
        treasury,
    }
}

fn paper(env: &Env) -> String {
    String::from_str(env, "crispr-nanomedicine-biotech")
}

#[test]
fn una_lectura_reparte_98_a_la_editorial_y_2_a_la_tesoreria() {
    let s = setup();
    let split = s
        .splitter
        .pay(&s.payer, &paper(&s.env), &s.publisher, &PRICE);

    assert_eq!(
        split,
        Split {
            publisher: 4_900_000,
            fee: 100_000
        }
    );
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(s.usdc.balance(&s.payer), 9 * PRICE);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        0,
        "el contrato no guarda fondos"
    );
}

#[test]
fn la_firma_del_lector_cubre_la_llamada_y_las_dos_transferencias() {
    let s = setup();
    let paper_id = paper(&s.env);
    s.splitter.pay(&s.payer, &paper_id, &s.publisher, &PRICE);

    let transfer = |to: &Address, amount: i128| AuthorizedInvocation {
        function: AuthorizedFunction::Contract((
            s.usdc.address.clone(),
            Symbol::new(&s.env, "transfer"),
            (s.payer.clone(), to.clone(), amount).into_val(&s.env),
        )),
        sub_invocations: std::vec![],
    };
    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.payer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "pay"),
                    (s.payer.clone(), paper_id, s.publisher.clone(), PRICE).into_val(&s.env),
                )),
                sub_invocations: std::vec![
                    transfer(&s.publisher, 4_900_000),
                    transfer(&s.treasury, 100_000)
                ],
            }
        )]
    );
}

#[test]
fn sin_la_firma_del_lector_no_se_cobra() {
    let s = setup();
    s.env.set_auths(&[]);

    assert!(s
        .splitter
        .try_pay(&s.payer, &paper(&s.env), &s.publisher, &PRICE)
        .is_err());
    assert_eq!(s.usdc.balance(&s.payer), 10 * PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
}

#[test]
fn cada_pago_deja_un_comprobante_con_el_articulo() {
    let s = setup();
    let paper_id = paper(&s.env);
    s.splitter.pay(&s.payer, &paper_id, &s.publisher, &PRICE);

    let expected = Paid {
        publisher: s.publisher.clone(),
        paper_id,
        payer: s.payer.clone(),
        amount: PRICE,
        fee: 100_000,
    };
    assert_eq!(
        s.env.events().all().filter_by_contract(&s.splitter.address),
        [expected.to_xdr(&s.env, &s.splitter.address)]
    );
}

#[test]
fn la_comision_se_redondea_a_favor_de_la_editorial() {
    let s = setup();
    // 7 stroops: 2% = 0.14 → 0. 51 stroops: 2% = 1.02 → 1.
    assert_eq!(
        s.splitter.split(&7),
        Split {
            publisher: 7,
            fee: 0
        }
    );
    assert_eq!(
        s.splitter.split(&51),
        Split {
            publisher: 50,
            fee: 1
        }
    );

    s.splitter.pay(&s.payer, &paper(&s.env), &s.publisher, &7);
    assert_eq!(s.usdc.balance(&s.publisher), 7);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
}

#[test]
fn rechaza_montos_no_positivos() {
    let s = setup();
    for amount in [0, -PRICE] {
        assert_eq!(
            s.splitter
                .try_pay(&s.payer, &paper(&s.env), &s.publisher, &amount),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    assert_eq!(s.usdc.balance(&s.payer), 10 * PRICE);
}

#[test]
fn rechaza_montos_que_desbordan_la_comision() {
    let s = setup();
    assert_eq!(
        s.splitter.try_split(&i128::MAX),
        Err(Ok(Error::AmountOverflow.into()))
    );
}

#[test]
fn rechaza_ids_de_articulo_vacios_o_demasiado_largos() {
    let s = setup();
    let empty = String::from_str(&s.env, "");
    let long = String::from_str(&s.env, &"a".repeat(65));
    for paper_id in [empty, long] {
        assert_eq!(
            s.splitter
                .try_pay(&s.payer, &paper_id, &s.publisher, &PRICE),
            Err(Ok(Error::InvalidPaperId.into()))
        );
    }
    let max = String::from_str(&s.env, &"a".repeat(64));
    assert!(s
        .splitter
        .try_pay(&s.payer, &max, &s.publisher, &PRICE)
        .is_ok());
}

#[test]
fn sin_saldo_no_se_mueve_nada() {
    let s = setup();
    let broke = Address::generate(&s.env);
    assert!(s
        .splitter
        .try_pay(&broke, &paper(&s.env), &s.publisher, &PRICE)
        .is_err());
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
}

#[test]
fn la_configuracion_queda_fija_al_desplegar() {
    let s = setup();
    assert_eq!(
        s.splitter.config(),
        Config {
            token: s.usdc.address.clone(),
            treasury: s.treasury.clone(),
            fee_bps: FEE_BPS
        }
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn no_se_despliega_con_una_comision_mayor_al_10_por_ciento() {
    let env = Env::default();
    let usdc = Address::generate(&env);
    let treasury = Address::generate(&env);
    env.register(Splitter, (usdc, treasury, MAX_FEE_BPS + 1));
}
