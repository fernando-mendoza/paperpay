#![cfg(test)]
extern crate std;

use super::{
    Config, DataKey, Error, Item, Paid, PassBought, RefSplit, Referred, Refunded, Renewed, Share,
    Split, Splitter, SplitterClient, Subscribed, Subscription, Tipped, Unsubscribed, DAY_SECONDS,
    MAX_ACCESS_DAYS, MAX_AUTHORS, MAX_BATCH_ITEMS, MAX_FEE_BPS, MAX_PASS_DAYS, MAX_RENEWALS,
};
use soroban_sdk::{
    testutils::{
        storage::Persistent as _, Address as _, AuthorizedFunction, AuthorizedInvocation,
        Events as _, Ledger as _,
    },
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env, Event, IntoVal, String, Symbol, Vec,
};

/// $0.50 USDC con 7 decimales.
const PRICE: i128 = 5_000_000;
const FEE_BPS: u32 = 200;
/// Un día de pase: 1 USDC.
const PASS_DAY_PRICE: i128 = 10_000_000;
/// Un día de un artículo abierto para todos: 5 USDC.
const OPEN_DAY_PRICE: i128 = 50_000_000;
/// Hora del ledger al empezar cada prueba (26-nov-2026 06:00 UTC).
const START: u64 = 1_795_672_800;

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
    // Los límites de TTL de testnet (protocolo 30), para que las pruebas de vencimiento valgan allá.
    env.ledger().with_mut(|li| {
        li.timestamp = START;
        li.sequence_number = 6_000_000;
        li.max_entry_ttl = 3_110_400;
        li.min_persistent_entry_ttl = 120_960;
        li.min_temp_entry_ttl = 720;
    });

    let issuer = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(issuer).address();
    let payer = Address::generate(&env);
    let publisher = Address::generate(&env);
    let treasury = Address::generate(&env);
    StellarAssetClient::new(&env, &usdc).mint(&payer, &(10 * PRICE));

    let id = env.register(
        Splitter,
        (
            usdc.clone(),
            treasury.clone(),
            FEE_BPS,
            PASS_DAY_PRICE,
            OPEN_DAY_PRICE,
        ),
    );
    Setup {
        splitter: SplitterClient::new(&env, &id),
        usdc: TokenClient::new(&env, &usdc),
        env,
        payer,
        publisher,
        treasury,
    }
}

impl Setup<'_> {
    fn fund(&self, who: &Address, amount: i128) {
        StellarAssetClient::new(&self.env, &self.usdc.address).mint(who, &amount);
    }

    fn transfer_auth(&self, from: &Address, to: &Address, amount: i128) -> AuthorizedInvocation {
        AuthorizedInvocation {
            function: AuthorizedFunction::Contract((
                self.usdc.address.clone(),
                Symbol::new(&self.env, "transfer"),
                (from.clone(), to.clone(), amount).into_val(&self.env),
            )),
            sub_invocations: std::vec![],
        }
    }

    /// Adelanta la hora del ledger (y la secuencia, a un ledger cada 5 s).
    fn advance(&self, seconds: u64) {
        self.env.ledger().with_mut(|li| {
            li.timestamp += seconds;
            li.sequence_number += (seconds / 5) as u32;
        });
    }

    fn now(&self) -> u64 {
        self.env.ledger().timestamp()
    }

    fn receipts(&self) -> soroban_sdk::testutils::ContractEvents {
        self.env
            .events()
            .all()
            .filter_by_contract(&self.splitter.address)
    }
}

fn paper(env: &Env) -> String {
    String::from_str(env, "crispr-nanomedicine-biotech")
}

fn share(account: &Address, bps: u32) -> Share {
    Share {
        account: account.clone(),
        bps,
    }
}

fn item(env: &Env, paper_id: &str, publisher: &Address, amount: i128) -> Item {
    Item {
        paper_id: String::from_str(env, paper_id),
        publisher: publisher.clone(),
        amount,
    }
}

// ---------- pay ----------

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
                    s.transfer_auth(&s.payer, &s.publisher, 4_900_000),
                    s.transfer_auth(&s.payer, &s.treasury, 100_000),
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
fn cada_pago_deja_un_comprobante_con_el_articulo_y_quien_lo_lee() {
    let s = setup();
    let paper_id = paper(&s.env);
    s.splitter.pay(&s.payer, &paper_id, &s.publisher, &PRICE);

    let expected = Paid {
        publisher: s.publisher.clone(),
        reader: s.payer.clone(),
        paper_id,
        payer: s.payer.clone(),
        amount: PRICE,
        fee: 100_000,
    };
    assert_eq!(s.receipts(), [expected.to_xdr(&s.env, &s.splitter.address)]);
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
            fee_bps: FEE_BPS,
            pass_day_price: PASS_DAY_PRICE,
            open_day_price: OPEN_DAY_PRICE,
        }
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn no_se_despliega_con_una_comision_mayor_al_10_por_ciento() {
    let env = Env::default();
    let usdc = Address::generate(&env);
    let treasury = Address::generate(&env);
    env.register(
        Splitter,
        (
            usdc,
            treasury,
            MAX_FEE_BPS + 1,
            PASS_DAY_PRICE,
            OPEN_DAY_PRICE,
        ),
    );
}

#[test]
fn no_se_despliega_con_precios_por_dia_en_cero_o_negativos() {
    for (pass, open) in [
        (0, OPEN_DAY_PRICE),
        (PASS_DAY_PRICE, 0),
        (-1, OPEN_DAY_PRICE),
    ] {
        let env = Env::default();
        let usdc = Address::generate(&env);
        let treasury = Address::generate(&env);
        let deployed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            env.register(Splitter, (usdc, treasury, FEE_BPS, pass, open));
        }));
        let panic = deployed.expect_err("no debió desplegarse");
        let message = panic
            .downcast_ref::<std::string::String>()
            .map(|m| m.as_str())
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap_or_default();
        assert!(
            message.contains("Error(Contract, #2)"),
            "pase {pass}, abierto {open}: {message}"
        );
    }
}

// ---------- pay_batch ----------

#[test]
fn un_paquete_cobra_varios_articulos_con_una_firma_y_agrupa_por_editorial() {
    let s = setup();
    let other = Address::generate(&s.env);
    let items = vec![
        &s.env,
        item(&s.env, "crispr-nanomedicine-biotech", &s.publisher, PRICE),
        item(&s.env, "gravitational-lensing-dark-matter", &other, PRICE),
        item(&s.env, "autonomous-ai-micropayments", &s.publisher, PRICE),
    ];
    let total = s.splitter.pay_batch(&s.payer, &items);
    // auths() y events() solo guardan la última invocación: se leen antes de consultar saldos.
    let auths = s.env.auths();

    assert_eq!(
        total,
        Split {
            publisher: 14_700_000,
            fee: 300_000
        }
    );
    assert_eq!(s.usdc.balance(&s.publisher), 9_800_000);
    assert_eq!(s.usdc.balance(&other), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 300_000);
    assert_eq!(s.usdc.balance(&s.payer), 7 * PRICE);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        0,
        "el contrato no guarda fondos"
    );

    // Una sola firma del lector, sobre la llamada con la lista completa.
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, s.payer);
    assert_eq!(
        auths[0].1.function,
        AuthorizedFunction::Contract((
            s.splitter.address.clone(),
            Symbol::new(&s.env, "pay_batch"),
            (s.payer.clone(), items).into_val(&s.env),
        ))
    );
    // Tres artículos, dos editoriales: dos transferencias a editoriales + una a la tesorería.
    assert_eq!(auths[0].1.sub_invocations.len(), 3);
}

#[test]
fn un_paquete_deja_un_comprobante_por_articulo_en_orden() {
    let s = setup();
    let other = Address::generate(&s.env);
    let items = vec![
        &s.env,
        item(&s.env, "crispr-nanomedicine-biotech", &s.publisher, PRICE),
        item(
            &s.env,
            "gravitational-lensing-dark-matter",
            &other,
            2 * PRICE,
        ),
    ];
    s.splitter.pay_batch(&s.payer, &items);

    let receipt = |it: &Item, fee: i128| {
        Paid {
            publisher: it.publisher.clone(),
            reader: s.payer.clone(),
            paper_id: it.paper_id.clone(),
            payer: s.payer.clone(),
            amount: it.amount,
            fee,
        }
        .to_xdr(&s.env, &s.splitter.address)
    };
    assert_eq!(
        s.receipts(),
        [
            receipt(&items.get(0).unwrap(), 100_000),
            receipt(&items.get(1).unwrap(), 200_000)
        ]
    );
}

#[test]
fn un_paquete_es_todo_o_nada() {
    let s = setup();
    let short = Address::generate(&s.env);
    s.fund(&short, 2 * PRICE);
    let items = vec![
        &s.env,
        item(&s.env, "a", &s.publisher, PRICE),
        item(&s.env, "b", &s.publisher, PRICE),
        item(&s.env, "c", &s.publisher, PRICE),
    ];

    assert!(s.splitter.try_pay_batch(&short, &items).is_err());
    assert_eq!(s.usdc.balance(&short), 2 * PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
}

#[test]
fn un_paquete_rechaza_listas_vacias_demasiado_grandes_o_con_repetidos() {
    let s = setup();
    let empty: Vec<Item> = Vec::new(&s.env);
    assert_eq!(
        s.splitter.try_pay_batch(&s.payer, &empty),
        Err(Ok(Error::EmptyBatch.into()))
    );

    let mut too_many: Vec<Item> = Vec::new(&s.env);
    for i in 0..=MAX_BATCH_ITEMS {
        too_many.push_back(item(&s.env, &std::format!("paper-{i}"), &s.publisher, 1));
    }
    assert_eq!(
        s.splitter.try_pay_batch(&s.payer, &too_many),
        Err(Ok(Error::BatchTooLarge.into()))
    );

    let repeated = vec![
        &s.env,
        item(&s.env, "a", &s.publisher, PRICE),
        item(&s.env, "a", &s.publisher, PRICE),
    ];
    assert_eq!(
        s.splitter.try_pay_batch(&s.payer, &repeated),
        Err(Ok(Error::DuplicatePaper.into()))
    );

    let bad_amount = vec![
        &s.env,
        item(&s.env, "a", &s.publisher, PRICE),
        item(&s.env, "b", &s.publisher, 0),
    ];
    assert_eq!(
        s.splitter.try_pay_batch(&s.payer, &bad_amount),
        Err(Ok(Error::InvalidAmount.into()))
    );

    let bad_id = vec![
        &s.env,
        item(&s.env, "a", &s.publisher, PRICE),
        item(&s.env, "", &s.publisher, PRICE),
    ];
    assert_eq!(
        s.splitter.try_pay_batch(&s.payer, &bad_id),
        Err(Ok(Error::InvalidPaperId.into()))
    );

    assert_eq!(s.usdc.balance(&s.payer), 10 * PRICE, "ningún intento cobró");
}

#[test]
fn un_paquete_acepta_el_maximo_de_articulos() {
    let s = setup();
    let mut items: Vec<Item> = Vec::new(&s.env);
    for i in 0..MAX_BATCH_ITEMS {
        items.push_back(item(
            &s.env,
            &std::format!("paper-{i}"),
            &s.publisher,
            PRICE,
        ));
    }
    let total = s.splitter.pay_batch(&s.payer, &items);
    assert_eq!(
        total,
        Split {
            publisher: 49_000_000,
            fee: 1_000_000
        }
    );
    assert_eq!(s.receipts().events().len(), MAX_BATCH_ITEMS as usize);
}

#[test]
fn la_comision_del_paquete_se_redondea_por_articulo() {
    let s = setup();
    // 49 stroops: 2% = 0.98 → 0 por artículo. Sobre la suma (147) serían 2.94 → 2: el paquete
    // cobra lo mismo que tres `pay` separados, y cada comprobante lleva la comisión de su artículo.
    let items = vec![
        &s.env,
        item(&s.env, "a", &s.publisher, 49),
        item(&s.env, "b", &s.publisher, 49),
        item(&s.env, "c", &s.publisher, 49),
    ];
    let total = s.splitter.pay_batch(&s.payer, &items);
    assert_eq!(
        total,
        Split {
            publisher: 147,
            fee: 0
        }
    );
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(s.usdc.balance(&s.publisher), 147);
}

#[test]
fn sin_la_firma_del_lector_no_hay_paquete() {
    let s = setup();
    s.env.set_auths(&[]);
    let items = vec![&s.env, item(&s.env, "a", &s.publisher, PRICE)];
    assert!(s.splitter.try_pay_batch(&s.payer, &items).is_err());
    assert_eq!(s.usdc.balance(&s.payer), 10 * PRICE);
}

// ---------- gift ----------

#[test]
fn un_regalo_cobra_a_quien_paga_y_da_el_acceso_a_otra_cuenta() {
    let s = setup();
    let student = Address::generate(&s.env);
    let paper_id = paper(&s.env);
    let split = s
        .splitter
        .gift(&s.payer, &student, &paper_id, &s.publisher, &PRICE);
    let receipts = s.receipts();

    assert_eq!(
        split,
        Split {
            publisher: 4_900_000,
            fee: 100_000
        }
    );
    assert_eq!(s.usdc.balance(&s.payer), 9 * PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(
        s.usdc.balance(&student),
        0,
        "el lector no recibe dinero, recibe el acceso"
    );

    let expected = Paid {
        publisher: s.publisher.clone(),
        reader: student,
        paper_id,
        payer: s.payer.clone(),
        amount: PRICE,
        fee: 100_000,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn el_regalo_solo_pide_la_firma_de_quien_paga() {
    let s = setup();
    let student = Address::generate(&s.env);
    let paper_id = paper(&s.env);
    s.splitter
        .gift(&s.payer, &student, &paper_id, &s.publisher, &PRICE);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.payer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "gift"),
                    (
                        s.payer.clone(),
                        student,
                        paper_id,
                        s.publisher.clone(),
                        PRICE
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![
                    s.transfer_auth(&s.payer, &s.publisher, 4_900_000),
                    s.transfer_auth(&s.payer, &s.treasury, 100_000),
                ],
            }
        )]
    );
}

#[test]
fn sin_la_firma_de_quien_paga_no_hay_regalo() {
    let s = setup();
    let student = Address::generate(&s.env);
    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_gift(&s.payer, &student, &paper(&s.env), &s.publisher, &PRICE)
        .is_err());
    assert_eq!(s.usdc.balance(&s.payer), 10 * PRICE);
}

// ---------- gift_batch ----------

#[test]
fn un_paquete_de_regalo_cobra_a_quien_paga_y_da_cada_articulo_a_otra_cuenta() {
    let s = setup();
    let student = Address::generate(&s.env);
    let other = Address::generate(&s.env);
    let items = vec![
        &s.env,
        item(&s.env, "crispr-nanomedicine-biotech", &s.publisher, PRICE),
        item(
            &s.env,
            "gravitational-lensing-dark-matter",
            &other,
            2 * PRICE,
        ),
    ];
    let total = s.splitter.gift_batch(&s.payer, &student, &items);
    let receipts = s.receipts();

    assert_eq!(
        total,
        Split {
            publisher: 14_700_000,
            fee: 300_000
        }
    );
    assert_eq!(s.usdc.balance(&s.payer), 7 * PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&other), 9_800_000);
    assert_eq!(s.usdc.balance(&s.treasury), 300_000);
    assert_eq!(
        s.usdc.balance(&student),
        0,
        "el lector no recibe dinero, recibe el acceso"
    );

    let receipt = |it: &Item, fee: i128| {
        Paid {
            publisher: it.publisher.clone(),
            reader: student.clone(),
            paper_id: it.paper_id.clone(),
            payer: s.payer.clone(),
            amount: it.amount,
            fee,
        }
        .to_xdr(&s.env, &s.splitter.address)
    };
    assert_eq!(
        receipts,
        [
            receipt(&items.get(0).unwrap(), 100_000),
            receipt(&items.get(1).unwrap(), 200_000)
        ]
    );
}

#[test]
fn el_paquete_de_regalo_solo_pide_la_firma_de_quien_paga() {
    let s = setup();
    let student = Address::generate(&s.env);
    let items = vec![
        &s.env,
        item(&s.env, "a", &s.publisher, PRICE),
        item(&s.env, "b", &s.publisher, PRICE),
    ];
    s.splitter.gift_batch(&s.payer, &student, &items);
    let auths = s.env.auths();

    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, s.payer);
    assert_eq!(
        auths[0].1.function,
        AuthorizedFunction::Contract((
            s.splitter.address.clone(),
            Symbol::new(&s.env, "gift_batch"),
            (s.payer.clone(), student, items).into_val(&s.env),
        ))
    );
    // Una editorial: una transferencia agrupada + la comisión.
    assert_eq!(
        auths[0].1.sub_invocations,
        std::vec![
            s.transfer_auth(&s.payer, &s.publisher, 9_800_000),
            s.transfer_auth(&s.payer, &s.treasury, 200_000),
        ]
    );
}

#[test]
fn el_paquete_de_regalo_tiene_las_mismas_reglas_que_el_paquete() {
    let s = setup();
    let student = Address::generate(&s.env);
    let empty: Vec<Item> = Vec::new(&s.env);
    assert_eq!(
        s.splitter.try_gift_batch(&s.payer, &student, &empty),
        Err(Ok(Error::EmptyBatch.into()))
    );

    let repeated = vec![
        &s.env,
        item(&s.env, "a", &s.publisher, PRICE),
        item(&s.env, "a", &s.publisher, PRICE),
    ];
    assert_eq!(
        s.splitter.try_gift_batch(&s.payer, &student, &repeated),
        Err(Ok(Error::DuplicatePaper.into()))
    );

    let mut too_many: Vec<Item> = Vec::new(&s.env);
    for i in 0..=MAX_BATCH_ITEMS {
        too_many.push_back(Item {
            paper_id: String::from_str(&s.env, &std::format!("paper-{i}")),
            publisher: s.publisher.clone(),
            amount: 1,
        });
    }
    assert_eq!(
        s.splitter.try_gift_batch(&s.payer, &student, &too_many),
        Err(Ok(Error::BatchTooLarge.into()))
    );

    // Todo o nada: el tercer artículo no alcanza y no se cobra ninguno.
    let short = Address::generate(&s.env);
    s.fund(&short, 2 * PRICE);
    let three = vec![
        &s.env,
        item(&s.env, "a", &s.publisher, PRICE),
        item(&s.env, "b", &s.publisher, PRICE),
        item(&s.env, "c", &s.publisher, PRICE),
    ];
    assert!(s.splitter.try_gift_batch(&short, &student, &three).is_err());
    assert_eq!(s.usdc.balance(&short), 2 * PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
}

#[test]
fn sin_la_firma_de_quien_paga_no_hay_paquete_de_regalo() {
    let s = setup();
    let student = Address::generate(&s.env);
    let items = vec![&s.env, item(&s.env, "a", &s.publisher, PRICE)];
    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_gift_batch(&s.payer, &student, &items)
        .is_err());
    assert_eq!(s.usdc.balance(&s.payer), 10 * PRICE);
}

// ---------- tip ----------

#[test]
fn una_propina_se_reparte_como_una_lectura_y_no_da_acceso() {
    let s = setup();
    let paper_id = paper(&s.env);
    let split = s.splitter.tip(&s.payer, &paper_id, &s.publisher, &PRICE);
    let receipts = s.receipts();

    assert_eq!(
        split,
        Split {
            publisher: 4_900_000,
            fee: 100_000
        }
    );
    assert_eq!(s.usdc.balance(&s.payer), 9 * PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        0,
        "el contrato no guarda fondos"
    );

    // Solo el evento `tipped`: ningún `paid`, así que la API no da acceso por una propina.
    let expected = Tipped {
        publisher: s.publisher.clone(),
        payer: s.payer.clone(),
        paper_id,
        amount: PRICE,
        fee: 100_000,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn la_propina_solo_pide_la_firma_de_quien_la_da() {
    let s = setup();
    let paper_id = paper(&s.env);
    s.splitter.tip(&s.payer, &paper_id, &s.publisher, &PRICE);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.payer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "tip"),
                    (s.payer.clone(), paper_id, s.publisher.clone(), PRICE).into_val(&s.env),
                )),
                sub_invocations: std::vec![
                    s.transfer_auth(&s.payer, &s.publisher, 4_900_000),
                    s.transfer_auth(&s.payer, &s.treasury, 100_000),
                ],
            }
        )]
    );
}

#[test]
fn la_propina_rechaza_montos_e_ids_invalidos_y_sin_firma_no_se_cobra() {
    let s = setup();
    let paper_id = paper(&s.env);
    assert_eq!(
        s.splitter.try_tip(&s.payer, &paper_id, &s.publisher, &0),
        Err(Ok(Error::InvalidAmount.into()))
    );
    assert_eq!(
        s.splitter.try_tip(
            &s.payer,
            &String::from_str(&s.env, ""),
            &s.publisher,
            &PRICE
        ),
        Err(Ok(Error::InvalidPaperId.into()))
    );

    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_tip(&s.payer, &paper_id, &s.publisher, &PRICE)
        .is_err());
    assert_eq!(s.usdc.balance(&s.payer), 10 * PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
}

// ---------- refund ----------

#[test]
fn una_devolucion_mueve_el_dinero_de_la_editorial_al_lector() {
    let s = setup();
    let paper_id = paper(&s.env);
    s.splitter.pay(&s.payer, &paper_id, &s.publisher, &PRICE);
    s.splitter
        .refund(&s.publisher, &s.payer, &paper_id, &4_900_000);
    let receipts = s.receipts();

    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(s.usdc.balance(&s.payer), 9 * PRICE + 4_900_000);
    assert_eq!(
        s.usdc.balance(&s.treasury),
        100_000,
        "la comisión no vuelve: la tesorería no firma"
    );

    let expected = Refunded {
        publisher: s.publisher.clone(),
        reader: s.payer.clone(),
        paper_id,
        amount: 4_900_000,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn la_devolucion_solo_la_firma_la_editorial() {
    let s = setup();
    let paper_id = paper(&s.env);
    s.fund(&s.publisher, PRICE);
    s.splitter.refund(&s.publisher, &s.payer, &paper_id, &PRICE);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.publisher.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "refund"),
                    (s.publisher.clone(), s.payer.clone(), paper_id, PRICE).into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(&s.publisher, &s.payer, PRICE)],
            }
        )]
    );
}

#[test]
fn sin_la_firma_de_la_editorial_no_hay_devolucion() {
    let s = setup();
    s.fund(&s.publisher, PRICE);
    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_refund(&s.publisher, &s.payer, &paper(&s.env), &PRICE)
        .is_err());
    assert_eq!(s.usdc.balance(&s.publisher), PRICE);
    assert_eq!(s.usdc.balance(&s.payer), 10 * PRICE);
}

#[test]
fn la_devolucion_rechaza_montos_e_ids_invalidos_y_no_pasa_del_saldo() {
    let s = setup();
    let paper_id = paper(&s.env);
    s.fund(&s.publisher, PRICE);
    assert_eq!(
        s.splitter.try_refund(&s.publisher, &s.payer, &paper_id, &0),
        Err(Ok(Error::InvalidAmount.into()))
    );
    assert_eq!(
        s.splitter
            .try_refund(&s.publisher, &s.payer, &paper_id, &-1),
        Err(Ok(Error::InvalidAmount.into()))
    );
    let long = String::from_str(&s.env, &"x".repeat(65));
    assert_eq!(
        s.splitter.try_refund(&s.publisher, &s.payer, &long, &PRICE),
        Err(Ok(Error::InvalidPaperId.into()))
    );

    assert!(s
        .splitter
        .try_refund(&s.publisher, &s.payer, &paper_id, &(PRICE + 1))
        .is_err());
    assert_eq!(s.usdc.balance(&s.publisher), PRICE);
    assert_eq!(s.usdc.balance(&s.payer), 10 * PRICE);
}

// ---------- pay_authors ----------

#[test]
fn una_lectura_con_coautores_reparte_la_parte_de_la_editorial_segun_cada_autor() {
    let s = setup();
    let paper_id = paper(&s.env);
    let (ana, beto, caro) = (
        Address::generate(&s.env),
        Address::generate(&s.env),
        Address::generate(&s.env),
    );
    let authors = vec![
        &s.env,
        share(&ana, 6_000),
        share(&beto, 3_000),
        share(&caro, 1_000),
    ];
    let split = s
        .splitter
        .pay_authors(&s.payer, &paper_id, &authors, &PRICE);
    let receipts = s.receipts();

    assert_eq!(
        split,
        Split {
            publisher: 4_900_000,
            fee: 100_000
        }
    );
    assert_eq!(s.usdc.balance(&ana), 2_940_000);
    assert_eq!(s.usdc.balance(&beto), 1_470_000);
    assert_eq!(s.usdc.balance(&caro), 490_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(s.usdc.balance(&s.payer), 9 * PRICE);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        0,
        "el contrato no guarda fondos"
    );

    // Un solo `paid`, a nombre del primer autor: da acceso a quien paga, igual que `pay`.
    let expected = Paid {
        publisher: ana,
        reader: s.payer.clone(),
        paper_id,
        payer: s.payer.clone(),
        amount: PRICE,
        fee: 100_000,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn el_residuo_de_los_coautores_va_al_primer_autor() {
    let s = setup();
    let (ana, beto, caro) = (
        Address::generate(&s.env),
        Address::generate(&s.env),
        Address::generate(&s.env),
    );
    let authors = vec![
        &s.env,
        share(&ana, 3_334),
        share(&beto, 3_333),
        share(&caro, 3_333),
    ];
    // 101 stroops: comisión 2 y 99 para los autores. 99 × 33.33% = 32.99 → 32 cada uno de los
    // dos últimos; el primero recibe 33 más el residuo de 2: 35. No se pierde ningún stroop.
    let split = s
        .splitter
        .pay_authors(&s.payer, &paper(&s.env), &authors, &101);

    assert_eq!(
        split,
        Split {
            publisher: 99,
            fee: 2
        }
    );
    assert_eq!(s.usdc.balance(&ana), 35);
    assert_eq!(s.usdc.balance(&beto), 32);
    assert_eq!(s.usdc.balance(&caro), 32);
    assert_eq!(s.usdc.balance(&s.treasury), 2);
    assert_eq!(s.usdc.balance(&s.payer), 10 * PRICE - 101);
}

#[test]
fn la_lectura_con_coautores_solo_pide_la_firma_de_quien_paga() {
    let s = setup();
    let paper_id = paper(&s.env);
    let (ana, beto) = (Address::generate(&s.env), Address::generate(&s.env));
    let authors = vec![&s.env, share(&ana, 7_500), share(&beto, 2_500)];
    s.splitter
        .pay_authors(&s.payer, &paper_id, &authors, &PRICE);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.payer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "pay_authors"),
                    (s.payer.clone(), paper_id, authors, PRICE).into_val(&s.env),
                )),
                sub_invocations: std::vec![
                    s.transfer_auth(&s.payer, &ana, 3_675_000),
                    s.transfer_auth(&s.payer, &beto, 1_225_000),
                    s.transfer_auth(&s.payer, &s.treasury, 100_000),
                ],
            }
        )]
    );
}

#[test]
fn los_coautores_rechazan_partes_invalidas_y_no_mueven_nada() {
    let s = setup();
    let paper_id = paper(&s.env);
    let (ana, beto) = (Address::generate(&s.env), Address::generate(&s.env));
    let invalid = |authors: Vec<Share>| {
        assert_eq!(
            s.splitter
                .try_pay_authors(&s.payer, &paper_id, &authors, &PRICE),
            Err(Ok(Error::InvalidShares.into()))
        );
    };

    invalid(Vec::new(&s.env));
    invalid(vec![&s.env, share(&ana, 10_000), share(&beto, 0)]);
    invalid(vec![&s.env, share(&ana, 5_000), share(&beto, 4_999)]);
    invalid(vec![&s.env, share(&ana, 5_000), share(&beto, 5_001)]);
    invalid(vec![&s.env, share(&ana, 5_000), share(&ana, 5_000)]);
    // u32::MAX + 10 001 da 10 000 si se suma sin revisar cada parte.
    invalid(vec![&s.env, share(&ana, u32::MAX), share(&beto, 10_001)]);

    let mut crowd: Vec<Share> = Vec::new(&s.env);
    for _ in 0..=MAX_AUTHORS {
        crowd.push_back(share(&Address::generate(&s.env), 1));
    }
    assert_eq!(
        s.splitter
            .try_pay_authors(&s.payer, &paper_id, &crowd, &PRICE),
        Err(Ok(Error::TooManyAuthors.into()))
    );

    let ok = vec![&s.env, share(&ana, 5_000), share(&beto, 5_000)];
    assert_eq!(
        s.splitter.try_pay_authors(&s.payer, &paper_id, &ok, &0),
        Err(Ok(Error::InvalidAmount.into()))
    );
    assert_eq!(
        s.splitter
            .try_pay_authors(&s.payer, &String::from_str(&s.env, ""), &ok, &PRICE),
        Err(Ok(Error::InvalidPaperId.into()))
    );

    assert_eq!(s.usdc.balance(&s.payer), 10 * PRICE);
    assert_eq!(s.usdc.balance(&ana), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
}

#[test]
fn la_lectura_con_coautores_acepta_el_maximo_de_autores() {
    let s = setup();
    let mut authors: Vec<Share> = Vec::new(&s.env);
    for _ in 0..MAX_AUTHORS {
        authors.push_back(share(&Address::generate(&s.env), 1_000));
    }
    s.splitter
        .pay_authors(&s.payer, &paper(&s.env), &authors, &PRICE);

    for author in authors.iter() {
        assert_eq!(s.usdc.balance(&author.account), 490_000);
    }
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
}

#[test]
fn sin_la_firma_de_quien_paga_no_hay_lectura_con_coautores() {
    let s = setup();
    let ana = Address::generate(&s.env);
    let authors = vec![&s.env, share(&ana, 10_000)];
    s.env.set_auths(&[]);

    assert!(s
        .splitter
        .try_pay_authors(&s.payer, &paper(&s.env), &authors, &PRICE)
        .is_err());
    assert_eq!(s.usdc.balance(&s.payer), 10 * PRICE);
    assert_eq!(s.usdc.balance(&ana), 0);
}

// ---------- pay_ref ----------

#[test]
fn una_lectura_recomendada_le_da_la_mitad_de_la_comision_a_quien_la_recomendo() {
    let s = setup();
    let paper_id = paper(&s.env);
    let referrer = Address::generate(&s.env);
    let split = s
        .splitter
        .pay_ref(&s.payer, &paper_id, &s.publisher, &PRICE, &referrer);
    let receipts = s.receipts();

    assert_eq!(
        split,
        RefSplit {
            publisher: 4_900_000,
            fee: 50_000,
            referral: 50_000
        }
    );
    assert_eq!(
        s.usdc.balance(&s.publisher),
        4_900_000,
        "la editorial recibe lo mismo que en pay"
    );
    assert_eq!(s.usdc.balance(&referrer), 50_000);
    assert_eq!(s.usdc.balance(&s.treasury), 50_000);
    assert_eq!(s.usdc.balance(&s.payer), 9 * PRICE);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        0,
        "el contrato no guarda fondos"
    );

    // `paid` (da acceso a quien paga, con la comisión total) y después `referred`.
    let paid = Paid {
        publisher: s.publisher.clone(),
        reader: s.payer.clone(),
        paper_id: paper_id.clone(),
        payer: s.payer.clone(),
        amount: PRICE,
        fee: 100_000,
    };
    let referred = Referred {
        referrer,
        payer: s.payer.clone(),
        paper_id,
        publisher: s.publisher.clone(),
        amount: PRICE,
        reward: 50_000,
    };
    assert_eq!(
        receipts,
        [
            paid.to_xdr(&s.env, &s.splitter.address),
            referred.to_xdr(&s.env, &s.splitter.address)
        ]
    );
}

#[test]
fn con_comision_impar_el_stroop_sobrante_queda_en_la_tesoreria() {
    let s = setup();
    let referrer = Address::generate(&s.env);
    // 150 stroops: comisión 3; 1.5 → 1 para quien recomendó y 2 para la tesorería.
    let split = s
        .splitter
        .pay_ref(&s.payer, &paper(&s.env), &s.publisher, &150, &referrer);

    assert_eq!(
        split,
        RefSplit {
            publisher: 147,
            fee: 2,
            referral: 1
        }
    );
    assert_eq!(s.usdc.balance(&s.publisher), 147);
    assert_eq!(s.usdc.balance(&referrer), 1);
    assert_eq!(s.usdc.balance(&s.treasury), 2);
}

#[test]
fn la_lectura_recomendada_solo_pide_la_firma_de_quien_paga() {
    let s = setup();
    let paper_id = paper(&s.env);
    let referrer = Address::generate(&s.env);
    s.splitter
        .pay_ref(&s.payer, &paper_id, &s.publisher, &PRICE, &referrer);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.payer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "pay_ref"),
                    (
                        s.payer.clone(),
                        paper_id,
                        s.publisher.clone(),
                        PRICE,
                        referrer.clone()
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![
                    s.transfer_auth(&s.payer, &s.publisher, 4_900_000),
                    s.transfer_auth(&s.payer, &referrer, 50_000),
                    s.transfer_auth(&s.payer, &s.treasury, 50_000),
                ],
            }
        )]
    );
}

#[test]
fn nadie_se_recomienda_a_si_mismo_y_los_datos_invalidos_no_mueven_nada() {
    let s = setup();
    let paper_id = paper(&s.env);
    let referrer = Address::generate(&s.env);
    assert_eq!(
        s.splitter
            .try_pay_ref(&s.payer, &paper_id, &s.publisher, &PRICE, &s.payer),
        Err(Ok(Error::SelfReferral.into()))
    );
    assert_eq!(
        s.splitter
            .try_pay_ref(&s.payer, &paper_id, &s.publisher, &0, &referrer),
        Err(Ok(Error::InvalidAmount.into()))
    );
    assert_eq!(
        s.splitter.try_pay_ref(
            &s.payer,
            &String::from_str(&s.env, ""),
            &s.publisher,
            &PRICE,
            &referrer
        ),
        Err(Ok(Error::InvalidPaperId.into()))
    );

    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_pay_ref(&s.payer, &paper_id, &s.publisher, &PRICE, &referrer)
        .is_err());

    assert_eq!(s.usdc.balance(&s.payer), 10 * PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(s.usdc.balance(&referrer), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
}

#[test]
fn la_editorial_puede_recomendar_su_propio_articulo() {
    let s = setup();
    let split = s
        .splitter
        .pay_ref(&s.payer, &paper(&s.env), &s.publisher, &PRICE, &s.publisher);

    assert_eq!(split.referral, 50_000);
    assert_eq!(s.usdc.balance(&s.publisher), 4_950_000);
    assert_eq!(s.usdc.balance(&s.treasury), 50_000);
}

// ---------- buy_pass ----------

const DAY: u64 = DAY_SECONDS;

fn pass_price(days: u32) -> i128 {
    PASS_DAY_PRICE * days as i128
}

#[test]
fn un_pase_cobra_dias_por_precio_reparte_98_2_y_guarda_el_vencimiento() {
    let s = setup();
    s.fund(&s.payer, pass_price(10));
    let before = s.usdc.balance(&s.payer);
    let until = s
        .splitter
        .buy_pass(&s.payer, &s.payer, &s.publisher, &7, &pass_price(7));
    let receipts = s.receipts();

    assert_eq!(until, START + 7 * DAY);
    assert_eq!(s.splitter.pass_until(&s.payer, &s.publisher), until);
    assert_eq!(s.usdc.balance(&s.payer), before - 70_000_000);
    assert_eq!(s.usdc.balance(&s.publisher), 68_600_000);
    assert_eq!(s.usdc.balance(&s.treasury), 1_400_000);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        0,
        "el contrato no guarda fondos"
    );

    let expected = PassBought {
        publisher: s.publisher.clone(),
        reader: s.payer.clone(),
        payer: s.payer.clone(),
        days: 7,
        amount: 70_000_000,
        fee: 1_400_000,
        valid_until: until,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn los_dias_se_suman_al_pase_vigente_y_si_vencio_cuentan_desde_ahora() {
    let s = setup();
    s.fund(&s.payer, pass_price(10));
    s.splitter
        .buy_pass(&s.payer, &s.payer, &s.publisher, &3, &pass_price(3));

    // Renueva a la mitad: los 2 días se suman a los 3 que tenía.
    s.advance(DAY);
    let until = s
        .splitter
        .buy_pass(&s.payer, &s.payer, &s.publisher, &2, &pass_price(2));
    assert_eq!(until, START + 5 * DAY);

    // Ya vencido: el día nuevo cuenta desde ahora, no desde el vencimiento viejo.
    s.advance(9 * DAY);
    let until = s
        .splitter
        .buy_pass(&s.payer, &s.payer, &s.publisher, &1, &pass_price(1));
    assert_eq!(until, s.now() + DAY);
    assert_eq!(until, START + 11 * DAY);
}

#[test]
fn un_pase_de_regalo_queda_a_nombre_de_quien_lee_y_vale_solo_con_esa_editorial() {
    let s = setup();
    s.fund(&s.payer, pass_price(10));
    let student = Address::generate(&s.env);
    let other_publisher = Address::generate(&s.env);
    let until = s
        .splitter
        .buy_pass(&s.payer, &student, &s.publisher, &7, &pass_price(7));

    assert_eq!(s.splitter.pass_until(&student, &s.publisher), until);
    assert_eq!(s.splitter.pass_until(&s.payer, &s.publisher), 0);
    assert_eq!(s.splitter.pass_until(&student, &other_publisher), 0);
    assert_eq!(s.usdc.balance(&student), 0, "quien lee no paga nada");
}

#[test]
fn el_pase_solo_pide_la_firma_de_quien_paga() {
    let s = setup();
    s.fund(&s.payer, pass_price(10));
    let student = Address::generate(&s.env);
    s.splitter
        .buy_pass(&s.payer, &student, &s.publisher, &2, &pass_price(2));

    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.payer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "buy_pass"),
                    (
                        s.payer.clone(),
                        student.clone(),
                        s.publisher.clone(),
                        2u32,
                        pass_price(2)
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![
                    s.transfer_auth(&s.payer, &s.publisher, 19_600_000),
                    s.transfer_auth(&s.payer, &s.treasury, 400_000),
                ],
            }
        )]
    );
}

#[test]
fn el_pase_rechaza_dias_precios_y_vencimientos_invalidos_sin_mover_nada() {
    let s = setup();
    s.fund(&s.payer, pass_price(100));
    let before = s.usdc.balance(&s.payer);
    let buy = |days: u32, amount: i128| {
        s.splitter
            .try_buy_pass(&s.payer, &s.payer, &s.publisher, &days, &amount)
    };

    assert_eq!(buy(0, 0), Err(Ok(Error::InvalidDays.into())));
    assert_eq!(
        buy(MAX_PASS_DAYS + 1, pass_price(MAX_PASS_DAYS + 1)),
        Err(Ok(Error::InvalidDays.into()))
    );
    assert_eq!(buy(7, pass_price(7) - 1), Err(Ok(Error::WrongPrice.into())));
    assert_eq!(buy(7, pass_price(6)), Err(Ok(Error::WrongPrice.into())));
    assert_eq!(s.usdc.balance(&s.payer), before);
    assert_eq!(s.splitter.pass_until(&s.payer, &s.publisher), 0);

    // Hasta MAX_ACCESS_DAYS (90) se acumula; un día más ya no.
    assert!(buy(31, pass_price(31)).is_ok());
    assert!(buy(31, pass_price(31)).is_ok());
    assert!(buy(28, pass_price(28)).is_ok());
    assert_eq!(
        s.splitter.pass_until(&s.payer, &s.publisher),
        START + MAX_ACCESS_DAYS as u64 * DAY
    );
    let spent = s.usdc.balance(&s.payer);
    assert_eq!(buy(1, pass_price(1)), Err(Ok(Error::TooLong.into())));
    assert_eq!(s.usdc.balance(&s.payer), spent);

    s.env.set_auths(&[]);
    assert!(buy(1, pass_price(1)).is_err());
}

#[test]
fn sin_la_firma_de_quien_paga_no_hay_pase() {
    let s = setup();
    s.fund(&s.payer, pass_price(10));
    let before = s.usdc.balance(&s.payer);
    s.env.set_auths(&[]);

    assert!(s
        .splitter
        .try_buy_pass(&s.payer, &s.payer, &s.publisher, &1, &pass_price(1))
        .is_err());
    assert_eq!(s.usdc.balance(&s.payer), before);
    assert_eq!(s.splitter.pass_until(&s.payer, &s.publisher), 0);
}

#[test]
fn la_entrada_del_pase_vive_en_la_red_hasta_que_vence() {
    let s = setup();
    s.fund(&s.payer, pass_price(31));
    s.splitter
        .buy_pass(&s.payer, &s.payer, &s.publisher, &31, &pass_price(31));

    let ttl = s.env.as_contract(&s.splitter.address, || {
        s.env
            .storage()
            .persistent()
            .get_ttl(&DataKey::Pass(s.payer.clone(), s.publisher.clone()))
    });
    // 31 días a un ledger cada 5 s, más una semana de margen.
    let needed = (31 * DAY / 5) as u32;
    assert!(ttl >= needed, "TTL {ttl} < {needed}");
    assert_eq!(ttl, needed + 7 * 17_280);
}

// ---------- subscribe / renew_subscription / cancel_subscription ----------

/// Ledgers de un día (uno cada 5 s).
const DAY_LEDGERS: u32 = 17_280;

#[test]
fn una_suscripcion_cobra_el_primer_periodo_y_autoriza_las_renovaciones() {
    let s = setup();
    s.fund(&s.payer, pass_price(40));
    let before = s.usdc.balance(&s.payer);
    let seq = s.env.ledger().sequence();
    let until = s
        .splitter
        .subscribe(&s.payer, &s.publisher, &7, &3, &pass_price(7));
    let receipts = s.receipts();

    // El primer periodo se cobra ya, con el reparto de una lectura.
    assert_eq!(until, START + 7 * DAY);
    assert_eq!(s.splitter.pass_until(&s.payer, &s.publisher), until);
    assert_eq!(s.usdc.balance(&s.payer), before - 70_000_000);
    assert_eq!(s.usdc.balance(&s.publisher), 68_600_000);
    assert_eq!(s.usdc.balance(&s.treasury), 1_400_000);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        0,
        "el contrato no guarda fondos"
    );

    // Las 3 renovaciones quedan autorizadas, no cobradas: el dinero sigue en la wallet.
    assert_eq!(
        s.usdc.allowance(&s.payer, &s.splitter.address),
        3 * pass_price(7)
    );
    // 7 días × 4 periodos + 1 día de margen.
    let allowance_until = seq + 29 * DAY_LEDGERS;
    assert_eq!(
        s.splitter.subscription(&s.payer),
        Some(Subscription {
            publisher: s.publisher.clone(),
            days: 7,
            amount: pass_price(7),
            renewals_left: 3,
            allowance_until,
        })
    );

    let pass = PassBought {
        publisher: s.publisher.clone(),
        reader: s.payer.clone(),
        payer: s.payer.clone(),
        days: 7,
        amount: pass_price(7),
        fee: 1_400_000,
        valid_until: until,
    };
    let subscribed = Subscribed {
        publisher: s.publisher.clone(),
        reader: s.payer.clone(),
        days: 7,
        amount: pass_price(7),
        renewals: 3,
        allowance_until,
    };
    assert_eq!(
        receipts,
        [
            pass.to_xdr(&s.env, &s.splitter.address),
            subscribed.to_xdr(&s.env, &s.splitter.address)
        ]
    );
}

#[test]
fn la_suscripcion_pide_una_sola_firma_para_el_cobro_y_la_autorizacion() {
    let s = setup();
    s.fund(&s.payer, pass_price(10));
    let seq = s.env.ledger().sequence();
    s.splitter
        .subscribe(&s.payer, &s.publisher, &1, &2, &pass_price(1));

    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.payer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "subscribe"),
                    (
                        s.payer.clone(),
                        s.publisher.clone(),
                        1u32,
                        2u32,
                        pass_price(1)
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![
                    s.transfer_auth(&s.payer, &s.publisher, 9_800_000),
                    s.transfer_auth(&s.payer, &s.treasury, 200_000),
                    AuthorizedInvocation {
                        function: AuthorizedFunction::Contract((
                            s.usdc.address.clone(),
                            Symbol::new(&s.env, "approve"),
                            (
                                s.payer.clone(),
                                s.splitter.address.clone(),
                                2 * pass_price(1),
                                seq + 4 * DAY_LEDGERS
                            )
                                .into_val(&s.env),
                        )),
                        sub_invocations: std::vec![],
                    },
                ],
            }
        )]
    );
}

#[test]
fn la_renovacion_cobra_con_la_autorizacion_sin_firma_de_quien_lee() {
    let s = setup();
    s.fund(&s.payer, pass_price(40));
    s.splitter
        .subscribe(&s.payer, &s.publisher, &7, &3, &pass_price(7));
    let before = s.usdc.balance(&s.payer);

    // Al pase le falta justo un día: ya toca. La envía otra cuenta, sin ninguna firma.
    s.advance(6 * DAY);
    s.env.set_auths(&[]);
    let until = s.splitter.renew_subscription(&s.payer);
    let receipts = s.receipts();

    assert!(s.env.auths().is_empty(), "la renovación no pidió firmas");
    assert_eq!(until, START + 14 * DAY);
    assert_eq!(s.splitter.pass_until(&s.payer, &s.publisher), until);
    assert_eq!(s.usdc.balance(&s.payer), before - pass_price(7));
    assert_eq!(s.usdc.balance(&s.publisher), 2 * 68_600_000);
    assert_eq!(s.usdc.balance(&s.treasury), 2 * 1_400_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(
        s.usdc.allowance(&s.payer, &s.splitter.address),
        2 * pass_price(7)
    );
    assert_eq!(s.splitter.subscription(&s.payer).unwrap().renewals_left, 2);

    let pass = PassBought {
        publisher: s.publisher.clone(),
        reader: s.payer.clone(),
        payer: s.payer.clone(),
        days: 7,
        amount: pass_price(7),
        fee: 1_400_000,
        valid_until: until,
    };
    let renewed = Renewed {
        publisher: s.publisher.clone(),
        reader: s.payer.clone(),
        days: 7,
        amount: pass_price(7),
        fee: 1_400_000,
        valid_until: until,
        renewals_left: 2,
    };
    assert_eq!(
        receipts,
        [
            pass.to_xdr(&s.env, &s.splitter.address),
            renewed.to_xdr(&s.env, &s.splitter.address)
        ]
    );
}

#[test]
fn no_se_renueva_antes_de_tiempo_ni_se_cobra_mas_de_un_periodo_adelantado() {
    let s = setup();
    s.fund(&s.payer, pass_price(40));
    s.splitter
        .subscribe(&s.payer, &s.publisher, &7, &3, &pass_price(7));
    let before = s.usdc.balance(&s.payer);

    // Faltan 1 día y 1 segundo: todavía no.
    s.advance(6 * DAY - 1);
    assert_eq!(
        s.splitter.try_renew_subscription(&s.payer),
        Err(Ok(Error::RenewTooEarly.into()))
    );
    s.advance(1);
    s.splitter.renew_subscription(&s.payer);
    // Recién renovado, al pase le faltan 8 días: otra renovación tendría que esperar.
    assert_eq!(
        s.splitter.try_renew_subscription(&s.payer),
        Err(Ok(Error::RenewTooEarly.into()))
    );
    assert_eq!(s.usdc.balance(&s.payer), before - pass_price(7));
    assert_eq!(s.splitter.subscription(&s.payer).unwrap().renewals_left, 2);
}

#[test]
fn con_la_ultima_renovacion_la_suscripcion_termina() {
    let s = setup();
    s.fund(&s.payer, pass_price(10));
    // Un día por periodo: al pase siempre le falta un día o menos, así que la primera
    // renovación se puede cobrar desde ya (un periodo por adelantado, no más).
    s.splitter
        .subscribe(&s.payer, &s.publisher, &1, &2, &pass_price(1));
    assert_eq!(s.splitter.renew_subscription(&s.payer), START + 2 * DAY);
    assert_eq!(
        s.splitter.try_renew_subscription(&s.payer),
        Err(Ok(Error::RenewTooEarly.into()))
    );

    s.advance(DAY);
    assert_eq!(s.splitter.renew_subscription(&s.payer), START + 3 * DAY);
    assert_eq!(s.splitter.subscription(&s.payer), None);
    assert_eq!(s.usdc.allowance(&s.payer, &s.splitter.address), 0);

    s.advance(2 * DAY);
    assert_eq!(
        s.splitter.try_renew_subscription(&s.payer),
        Err(Ok(Error::NoSubscription.into()))
    );
}

#[test]
fn cancelar_borra_la_suscripcion_y_la_autorizacion_y_el_pase_pagado_sigue() {
    let s = setup();
    s.fund(&s.payer, pass_price(40));
    let until = s
        .splitter
        .subscribe(&s.payer, &s.publisher, &7, &3, &pass_price(7));

    s.env.set_auths(&[]);
    assert!(
        s.splitter.try_cancel_subscription(&s.payer).is_err(),
        "solo quien lee cancela"
    );
    s.env.mock_all_auths();

    s.splitter.cancel_subscription(&s.payer);
    let receipts = s.receipts();
    assert_eq!(s.splitter.subscription(&s.payer), None);
    assert_eq!(s.usdc.allowance(&s.payer, &s.splitter.address), 0);
    assert_eq!(s.splitter.pass_until(&s.payer, &s.publisher), until);
    assert_eq!(
        receipts,
        [Unsubscribed {
            publisher: s.publisher.clone(),
            reader: s.payer.clone(),
            renewals_left: 3,
        }
        .to_xdr(&s.env, &s.splitter.address)]
    );

    s.advance(6 * DAY);
    assert_eq!(
        s.splitter.try_renew_subscription(&s.payer),
        Err(Ok(Error::NoSubscription.into()))
    );
    assert_eq!(
        s.splitter.try_cancel_subscription(&s.payer),
        Err(Ok(Error::NoSubscription.into()))
    );
}

#[test]
fn la_suscripcion_rechaza_datos_invalidos_sin_mover_nada() {
    let s = setup();
    s.fund(&s.payer, pass_price(200));
    let before = s.usdc.balance(&s.payer);
    let subscribe = |days: u32, renewals: u32, amount: i128| {
        s.splitter
            .try_subscribe(&s.payer, &s.publisher, &days, &renewals, &amount)
    };

    assert_eq!(
        subscribe(7, 0, pass_price(7)),
        Err(Ok(Error::InvalidRenewals.into()))
    );
    assert_eq!(
        subscribe(1, MAX_RENEWALS + 1, pass_price(1)),
        Err(Ok(Error::InvalidRenewals.into()))
    );
    // 31 días × 5 periodos = 155 > 150.
    assert_eq!(
        subscribe(31, 4, pass_price(31)),
        Err(Ok(Error::TooLong.into()))
    );
    assert_eq!(
        subscribe(7, 3, pass_price(7) + 1),
        Err(Ok(Error::WrongPrice.into()))
    );
    assert_eq!(subscribe(0, 3, 0), Err(Ok(Error::InvalidDays.into())));
    assert_eq!(s.usdc.balance(&s.payer), before);
    assert_eq!(s.splitter.subscription(&s.payer), None);
    assert_eq!(s.usdc.allowance(&s.payer, &s.splitter.address), 0);

    // 30 días × 5 periodos = 150: el máximo entra.
    assert!(subscribe(30, 4, pass_price(30)).is_ok());

    s.env.set_auths(&[]);
    let other = Address::generate(&s.env);
    assert!(s
        .splitter
        .try_subscribe(&other, &s.publisher, &1, &1, &pass_price(1))
        .is_err());
    assert_eq!(s.splitter.subscription(&other), None);
}

#[test]
fn sin_autorizacion_o_sin_saldo_la_renovacion_falla_y_no_cambia_nada() {
    let s = setup();
    s.fund(&s.payer, pass_price(40));
    let until = s
        .splitter
        .subscribe(&s.payer, &s.publisher, &7, &3, &pass_price(7));
    s.advance(6 * DAY);

    // Quien lee retira la autorización directo en el token.
    s.usdc.approve(
        &s.payer,
        &s.splitter.address,
        &0,
        &s.env.ledger().sequence(),
    );
    assert!(s.splitter.try_renew_subscription(&s.payer).is_err());
    assert_eq!(s.splitter.pass_until(&s.payer, &s.publisher), until);
    assert_eq!(s.splitter.subscription(&s.payer).unwrap().renewals_left, 3);

    // Con la autorización de vuelta pero sin saldo, tampoco.
    s.usdc.approve(
        &s.payer,
        &s.splitter.address,
        &(3 * pass_price(7)),
        &(s.env.ledger().sequence() + 100),
    );
    let elsewhere = Address::generate(&s.env);
    s.usdc
        .transfer(&s.payer, &elsewhere, &s.usdc.balance(&s.payer));
    assert!(s.splitter.try_renew_subscription(&s.payer).is_err());
    assert_eq!(s.splitter.pass_until(&s.payer, &s.publisher), until);
    assert_eq!(s.splitter.subscription(&s.payer).unwrap().renewals_left, 3);
}

#[test]
fn una_suscripcion_nueva_reemplaza_a_la_anterior_y_su_autorizacion() {
    let s = setup();
    s.fund(&s.payer, pass_price(40));
    s.splitter
        .subscribe(&s.payer, &s.publisher, &7, &3, &pass_price(7));
    let until = s
        .splitter
        .subscribe(&s.payer, &s.publisher, &1, &2, &pass_price(1));

    assert_eq!(until, START + 8 * DAY, "el pase pagado se conserva");
    let sub = s.splitter.subscription(&s.payer).unwrap();
    assert_eq!(
        (sub.days, sub.amount, sub.renewals_left),
        (1, pass_price(1), 2)
    );
    assert_eq!(
        s.usdc.allowance(&s.payer, &s.splitter.address),
        2 * pass_price(1)
    );
}

#[test]
fn la_suscripcion_vive_en_la_red_hasta_su_ultima_renovacion() {
    let s = setup();
    s.fund(&s.payer, pass_price(40));
    s.splitter
        .subscribe(&s.payer, &s.publisher, &30, &4, &pass_price(30));

    let ttl = s.env.as_contract(&s.splitter.address, || {
        s.env
            .storage()
            .persistent()
            .get_ttl(&DataKey::Sub(s.payer.clone()))
    });
    // 150 días + 1 de margen, a un ledger cada 5 s, más una semana.
    assert_eq!(ttl, 151 * DAY_LEDGERS + 7 * DAY_LEDGERS);
    assert!(ttl <= 3_110_400, "cabe en el TTL máximo de testnet");
}
