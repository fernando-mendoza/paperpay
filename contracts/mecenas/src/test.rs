#![cfg(test)]
extern crate std;

use super::{
    Config, DataKey, Error, Mecenas, MecenasClient, ReadTaken, ReadsLeft, Split, Stream,
    StreamCancelled, StreamOpened, StreamWithdrawn, DAY_SECONDS, MAX_FEE_BPS, MAX_READS_PER_GIFT,
    MAX_STREAM_SECONDS,
};
use soroban_sdk::{
    testutils::{
        storage::Persistent as _, Address as _, AuthorizedFunction, AuthorizedInvocation,
        Events as _, Ledger as _,
    },
    token::{StellarAssetClient, TokenClient},
    Address, Env, Event, IntoVal, String, Symbol,
};

const FEE_BPS: u32 = 200;
/// Una lectura: 0.50 USDC con 7 decimales.
const PRICE: i128 = 5_000_000;
/// El flujo de prueba: 1 USDC en 1 000 segundos (10 000 stroops por segundo).
const GRANT: i128 = 10_000_000;
const SPAN: u64 = 1_000;
const DAY: u64 = DAY_SECONDS;
const DAY_LEDGERS: u32 = 17_280;
/// Hora del ledger al empezar cada prueba (27-nov-2026 06:00 UTC).
const START: u64 = 1_795_759_200;

struct Setup<'a> {
    env: Env,
    mecenas: MecenasClient<'a>,
    usdc: TokenClient<'a>,
    treasury: Address,
}

fn setup() -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();
    // Los límites de TTL de testnet, para que las pruebas de vencimiento valgan allá.
    env.ledger().with_mut(|li| {
        li.timestamp = START;
        li.sequence_number = 6_000_000;
        li.max_entry_ttl = 3_110_400;
        li.min_persistent_entry_ttl = 120_960;
        li.min_temp_entry_ttl = 720;
    });
    let issuer = Address::generate(&env);
    let usdc = env.register_stellar_asset_contract_v2(issuer).address();
    let treasury = Address::generate(&env);
    let id = env.register(Mecenas, (usdc.clone(), treasury.clone(), FEE_BPS));
    Setup {
        mecenas: MecenasClient::new(&env, &id),
        usdc: TokenClient::new(&env, &usdc),
        env,
        treasury,
    }
}

impl Setup<'_> {
    /// Una cuenta con `amount` de USDC.
    fn funded(&self, amount: i128) -> Address {
        let who = Address::generate(&self.env);
        StellarAssetClient::new(&self.env, &self.usdc.address).mint(&who, &amount);
        who
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
            .filter_by_contract(&self.mecenas.address)
    }

    fn held(&self) -> i128 {
        self.usdc.balance(&self.mecenas.address)
    }

    /// Un flujo de 1 USDC en 1 000 s. Devuelve el mecenas, quien recibe y el id.
    fn opened(&self) -> (Address, Address, u32) {
        let patron = self.funded(GRANT);
        let lab = Address::generate(&self.env);
        let id = self.mecenas.open_stream(&patron, &lab, &GRANT, &SPAN);
        (patron, lab, id)
    }
}

fn paper(env: &Env) -> String {
    String::from_str(env, "crispr-nanomedicine-biotech")
}

// ---------- despliegue ----------

#[test]
fn la_configuracion_queda_fija_al_desplegar_y_la_comision_tiene_tope() {
    let s = setup();
    assert_eq!(
        s.mecenas.config(),
        Config {
            token: s.usdc.address.clone(),
            treasury: s.treasury.clone(),
            fee_bps: FEE_BPS,
        }
    );

    // Con el 10% exacto sí se despliega.
    s.env.register(
        Mecenas,
        (s.usdc.address.clone(), s.treasury.clone(), MAX_FEE_BPS),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn no_se_despliega_con_una_comision_mayor_al_10_por_ciento() {
    let env = Env::default();
    let usdc = Address::generate(&env);
    let treasury = Address::generate(&env);
    env.register(Mecenas, (usdc, treasury, MAX_FEE_BPS + 1));
}

// ---------- open_stream ----------

#[test]
fn abrir_un_flujo_deja_el_monto_en_el_contrato_y_fija_cuando_empieza_y_termina() {
    let s = setup();
    let patron = s.funded(2 * GRANT);
    let lab = Address::generate(&s.env);
    let id = s.mecenas.open_stream(&patron, &lab, &GRANT, &SPAN);
    let receipts = s.receipts();
    let auths = s.env.auths();

    assert_eq!(id, 0);
    assert_eq!(s.usdc.balance(&patron), GRANT);
    assert_eq!(s.held(), GRANT, "el flujo queda en el contrato");
    assert_eq!(s.usdc.balance(&lab), 0);
    assert_eq!(
        s.mecenas.stream(&id),
        Some(Stream {
            patron: patron.clone(),
            beneficiary: lab.clone(),
            amount: GRANT,
            start: START,
            end: START + SPAN,
            withdrawn: 0,
        })
    );
    let expected = StreamOpened {
        patron: patron.clone(),
        beneficiary: lab.clone(),
        stream_id: 0,
        amount: GRANT,
        start: START,
        end: START + SPAN,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.mecenas.address)]);
    assert_eq!(
        auths,
        std::vec![(
            patron.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.mecenas.address.clone(),
                    Symbol::new(&s.env, "open_stream"),
                    (patron.clone(), lab.clone(), GRANT, SPAN).into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(&patron, &s.mecenas.address, GRANT)],
            }
        )]
    );

    // Los ids son secuenciales y cada flujo guarda lo suyo.
    let second = s.mecenas.open_stream(&patron, &lab, &4_000_000, &DAY);
    assert_eq!(second, 1);
    assert_eq!(s.mecenas.stream(&second).unwrap().end, START + DAY);
    assert_eq!(s.mecenas.stream(&id).unwrap().amount, GRANT);
    assert_eq!(s.mecenas.stream(&2), None);
    assert_eq!(s.held(), GRANT + 4_000_000);
}

#[test]
fn abrir_rechaza_montos_y_duraciones_invalidos_y_sin_firma_no_hay_flujo() {
    let s = setup();
    let patron = s.funded(3 * GRANT);
    let lab = Address::generate(&s.env);
    let open =
        |amount: i128, duration: u64| s.mecenas.try_open_stream(&patron, &lab, &amount, &duration);

    for amount in [0, -1] {
        assert_eq!(open(amount, SPAN), Err(Ok(Error::InvalidAmount.into())));
    }
    for duration in [0, MAX_STREAM_SECONDS + 1] {
        assert_eq!(
            open(GRANT, duration),
            Err(Ok(Error::InvalidDuration.into()))
        );
    }
    assert_eq!(open(i128::MAX, 2), Err(Ok(Error::AmountOverflow.into())));
    assert_eq!(s.held(), 0);
    assert_eq!(s.mecenas.stream(&0), None);

    // Los bordes sí valen: un segundo y el máximo.
    assert!(open(GRANT, 1).is_ok());
    assert!(open(GRANT, MAX_STREAM_SECONDS).is_ok());

    s.env.set_auths(&[]);
    assert!(open(GRANT, SPAN).is_err());
    assert_eq!(s.usdc.balance(&patron), GRANT);
    assert_eq!(s.mecenas.stream(&2), None);
}

// ---------- withdraw_stream ----------

#[test]
fn quien_recibe_retira_lo_devengado_y_la_tesoreria_cobra_el_2() {
    let s = setup();
    let (patron, lab, id) = s.opened();

    // A la mitad del periodo se devengó la mitad.
    s.advance(SPAN / 2);
    let split = s.mecenas.withdraw_stream(&lab, &id);
    let receipts = s.receipts();
    let auths = s.env.auths();
    assert_eq!(
        split,
        Split {
            net: 4_900_000,
            fee: 100_000
        }
    );
    assert_eq!(s.usdc.balance(&lab), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(s.held(), GRANT / 2);
    assert_eq!(s.mecenas.stream(&id).unwrap().withdrawn, GRANT / 2);
    let expected = StreamWithdrawn {
        beneficiary: lab.clone(),
        stream_id: id,
        amount: GRANT / 2,
        fee: 100_000,
        withdrawn: GRANT / 2,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.mecenas.address)]);
    // Solo firma quien recibe, y su firma no autoriza ninguna transferencia suya.
    assert_eq!(
        auths,
        std::vec![(
            lab.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.mecenas.address.clone(),
                    Symbol::new(&s.env, "withdraw_stream"),
                    (lab.clone(), id).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );

    // En el mismo segundo no hay nada nuevo.
    assert_eq!(
        s.mecenas.try_withdraw_stream(&lab, &id),
        Err(Ok(Error::NothingToWithdraw.into()))
    );

    // Terminado el flujo (y mucho después), retira el resto.
    s.advance(10 * DAY);
    s.mecenas.withdraw_stream(&lab, &id);
    assert_eq!(s.usdc.balance(&lab), 9_800_000);
    assert_eq!(s.usdc.balance(&s.treasury), 200_000);
    assert_eq!(s.usdc.balance(&patron), 0);
    assert_eq!(s.held(), 0);
    assert_eq!(
        s.mecenas.try_withdraw_stream(&lab, &id),
        Err(Ok(Error::NothingToWithdraw.into()))
    );
}

#[test]
fn lo_devengado_se_redondea_hacia_abajo_y_al_final_no_se_pierde_nada() {
    let s = setup();
    let patron = s.funded(10);
    let lab = Address::generate(&s.env);
    // 10 stroops en 3 segundos: 3 al primer segundo, 6 al segundo y los 10 al tercero.
    let id = s.mecenas.open_stream(&patron, &lab, &10, &3);

    assert_eq!(
        s.mecenas.try_withdraw_stream(&lab, &id),
        Err(Ok(Error::NothingToWithdraw.into())),
        "recién abierto no hay nada devengado"
    );
    s.advance(1);
    s.mecenas.withdraw_stream(&lab, &id);
    assert_eq!(s.mecenas.stream(&id).unwrap().withdrawn, 3);
    s.advance(1);
    s.mecenas.withdraw_stream(&lab, &id);
    assert_eq!(s.mecenas.stream(&id).unwrap().withdrawn, 6);
    s.advance(1);
    s.mecenas.withdraw_stream(&lab, &id);
    assert_eq!(s.mecenas.stream(&id).unwrap().withdrawn, 10);
    // Con montos tan chicos la comisión se redondea a cero y todo queda para quien recibe.
    assert_eq!(s.usdc.balance(&lab), 10);
    assert_eq!(s.held(), 0);
}

#[test]
fn solo_retira_quien_recibe_y_solo_con_su_firma() {
    let s = setup();
    let (patron, lab, id) = s.opened();
    let stranger = Address::generate(&s.env);
    s.advance(SPAN);

    // Ni el mecenas puede sacar lo que ya se devengó.
    for who in [&patron, &stranger] {
        assert_eq!(
            s.mecenas.try_withdraw_stream(who, &id),
            Err(Ok(Error::NoStream.into()))
        );
    }
    assert_eq!(
        s.mecenas.try_withdraw_stream(&lab, &99),
        Err(Ok(Error::NoStream.into()))
    );
    s.env.set_auths(&[]);
    assert!(s.mecenas.try_withdraw_stream(&lab, &id).is_err());
    assert_eq!(s.held(), GRANT);
    assert_eq!(s.mecenas.stream(&id).unwrap().withdrawn, 0);
}

// ---------- cancel_stream ----------

#[test]
fn cortar_el_flujo_devuelve_lo_no_devengado_y_lo_devengado_sigue_siendo_de_quien_recibe() {
    let s = setup();
    let (patron, lab, id) = s.opened();

    // A un cuarto del periodo: 0.25 devengado, 0.75 de vuelta.
    s.advance(SPAN / 4);
    let refund = s.mecenas.cancel_stream(&patron, &id);
    let receipts = s.receipts();
    let auths = s.env.auths();
    assert_eq!(refund, 7_500_000);
    assert_eq!(s.usdc.balance(&patron), 7_500_000, "completo, sin comisión");
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(s.held(), 2_500_000, "lo devengado sigue en el contrato");
    assert_eq!(
        s.mecenas.stream(&id),
        Some(Stream {
            patron: patron.clone(),
            beneficiary: lab.clone(),
            amount: 2_500_000,
            start: START,
            end: START + SPAN / 4,
            withdrawn: 0,
        })
    );
    let expected = StreamCancelled {
        patron: patron.clone(),
        stream_id: id,
        refund: 7_500_000,
        kept: 2_500_000,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.mecenas.address)]);
    assert_eq!(
        auths,
        std::vec![(
            patron.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.mecenas.address.clone(),
                    Symbol::new(&s.env, "cancel_stream"),
                    (patron.clone(), id).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );

    // Cortado, ya no se devenga más: aunque pase el tiempo, quien recibe retira solo lo suyo.
    s.advance(10 * SPAN);
    s.mecenas.withdraw_stream(&lab, &id);
    assert_eq!(s.usdc.balance(&lab), 2_450_000);
    assert_eq!(s.usdc.balance(&s.treasury), 50_000);
    assert_eq!(s.held(), 0);
    assert_eq!(
        s.mecenas.try_cancel_stream(&patron, &id),
        Err(Ok(Error::StreamEnded.into()))
    );
}

#[test]
fn cortar_despues_de_un_retiro_reparte_exacto_y_un_flujo_terminado_no_se_corta() {
    let s = setup();
    let (patron, lab, id) = s.opened();
    let (other_patron, _, finished) = s.opened();

    s.advance(SPAN / 5);
    s.mecenas.withdraw_stream(&lab, &id);
    s.advance(SPAN / 5);
    assert_eq!(s.mecenas.cancel_stream(&patron, &id), 6_000_000);
    s.mecenas.withdraw_stream(&lab, &id);
    // 0.40 devengado en total: 0.392 para quien recibe y 0.008 de comisión, en dos retiros.
    assert_eq!(s.usdc.balance(&lab), 3_920_000);
    assert_eq!(s.usdc.balance(&s.treasury), 80_000);
    assert_eq!(s.usdc.balance(&patron), 6_000_000);

    // Un segundo antes del final todavía se corta; en el segundo final, ya no.
    s.advance(SPAN - 2 * (SPAN / 5) - 1);
    let (late_patron, _, late) = s.opened();
    assert_eq!(s.mecenas.cancel_stream(&other_patron, &finished), 10_000);
    s.advance(SPAN);
    assert_eq!(
        s.mecenas.try_cancel_stream(&late_patron, &late),
        Err(Ok(Error::StreamEnded.into()))
    );
    assert_eq!(s.usdc.balance(&late_patron), 0);
}

#[test]
fn cortar_en_el_mismo_segundo_en_que_se_abrio_devuelve_todo() {
    let s = setup();
    let (patron, lab, id) = s.opened();

    assert_eq!(s.mecenas.cancel_stream(&patron, &id), GRANT);
    assert_eq!(s.usdc.balance(&patron), GRANT);
    assert_eq!(s.held(), 0);
    // El flujo quedó con duración cero: retirar en ese mismo segundo no divide entre cero.
    assert_eq!(
        s.mecenas.try_withdraw_stream(&lab, &id),
        Err(Ok(Error::NothingToWithdraw.into()))
    );
    s.advance(DAY);
    assert_eq!(
        s.mecenas.try_withdraw_stream(&lab, &id),
        Err(Ok(Error::NothingToWithdraw.into()))
    );
}

#[test]
fn solo_el_mecenas_corta_su_flujo_y_solo_con_su_firma() {
    let s = setup();
    let (patron, lab, id) = s.opened();
    let stranger = Address::generate(&s.env);
    s.advance(SPAN / 2);

    // Quien recibe no puede cortarlo (ni cobrarse lo no devengado).
    for who in [&lab, &stranger] {
        assert_eq!(
            s.mecenas.try_cancel_stream(who, &id),
            Err(Ok(Error::NoStream.into()))
        );
    }
    assert_eq!(
        s.mecenas.try_cancel_stream(&patron, &99),
        Err(Ok(Error::NoStream.into()))
    );
    s.env.set_auths(&[]);
    assert!(s.mecenas.try_cancel_stream(&patron, &id).is_err());
    assert_eq!(s.held(), GRANT);
    assert_eq!(s.mecenas.stream(&id).unwrap().end, START + SPAN);
}

// ---------- leave_read ----------

#[test]
fn dejar_lecturas_pagadas_deposita_el_total_y_suma_a_las_pendientes() {
    let s = setup();
    let donor = s.funded(5 * PRICE);
    let publisher = Address::generate(&s.env);
    let pending = s.mecenas.leave_read(&donor, &publisher, &PRICE, &3);
    let receipts = s.receipts();
    let auths = s.env.auths();

    assert_eq!(pending, 3);
    assert_eq!(s.usdc.balance(&donor), 2 * PRICE);
    assert_eq!(s.held(), 3 * PRICE, "quedan en el contrato");
    assert_eq!(s.usdc.balance(&publisher), 0);
    assert_eq!(s.mecenas.pending_reads(&publisher, &PRICE), 3);
    let expected = ReadsLeft {
        donor: donor.clone(),
        publisher: publisher.clone(),
        price: PRICE,
        count: 3,
        pending: 3,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.mecenas.address)]);
    assert_eq!(
        auths,
        std::vec![(
            donor.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.mecenas.address.clone(),
                    Symbol::new(&s.env, "leave_read"),
                    (donor.clone(), publisher.clone(), PRICE, 3u32).into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(&donor, &s.mecenas.address, 3 * PRICE)],
            }
        )]
    );

    // Otra persona suma a las mismas; otro precio y otra editorial van aparte.
    let second = s.funded(3 * PRICE);
    assert_eq!(s.mecenas.leave_read(&second, &publisher, &PRICE, &2), 5);
    assert_eq!(s.mecenas.leave_read(&second, &publisher, &2_000_000, &1), 1);
    assert_eq!(s.mecenas.pending_reads(&publisher, &PRICE), 5);
    assert_eq!(s.mecenas.pending_reads(&second, &PRICE), 0);
    assert_eq!(s.held(), 5 * PRICE + 2_000_000);
}

#[test]
fn dejar_rechaza_precios_y_cantidades_invalidos_y_sin_firma_no_se_deja_nada() {
    let s = setup();
    let donor = s.funded(100 * PRICE);
    let publisher = Address::generate(&s.env);
    let leave =
        |price: i128, count: u32| s.mecenas.try_leave_read(&donor, &publisher, &price, &count);

    for price in [0, -1] {
        assert_eq!(leave(price, 1), Err(Ok(Error::InvalidAmount.into())));
    }
    for count in [0, MAX_READS_PER_GIFT + 1] {
        assert_eq!(leave(PRICE, count), Err(Ok(Error::InvalidCount.into())));
    }
    assert_eq!(leave(i128::MAX, 2), Err(Ok(Error::AmountOverflow.into())));
    s.env.set_auths(&[]);
    assert!(leave(PRICE, 1).is_err());
    s.env.mock_all_auths();
    assert_eq!(s.held(), 0);
    assert_eq!(s.mecenas.pending_reads(&publisher, &PRICE), 0);

    // Los bordes sí valen: una lectura y el máximo por llamada.
    assert_eq!(leave(PRICE, 1), Ok(Ok(1)));
    assert_eq!(
        leave(PRICE, MAX_READS_PER_GIFT),
        Ok(Ok(MAX_READS_PER_GIFT + 1))
    );
}

// ---------- take_read ----------

#[test]
fn un_lector_toma_una_lectura_pendiente_sin_pagar_y_la_editorial_cobra() {
    let s = setup();
    let donor = s.funded(2 * PRICE);
    let publisher = Address::generate(&s.env);
    let reader = Address::generate(&s.env);
    s.mecenas.leave_read(&donor, &publisher, &PRICE, &2);
    let split = s
        .mecenas
        .take_read(&reader, &publisher, &paper(&s.env), &PRICE);
    let receipts = s.receipts();
    let auths = s.env.auths();

    assert_eq!(
        split,
        Split {
            net: 4_900_000,
            fee: 100_000
        }
    );
    assert_eq!(s.usdc.balance(&publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(
        s.usdc.balance(&reader),
        0,
        "quien lee no paga ni recibe nada"
    );
    assert_eq!(s.held(), PRICE);
    assert_eq!(s.mecenas.pending_reads(&publisher, &PRICE), 1);
    let expected = ReadTaken {
        publisher: publisher.clone(),
        reader: reader.clone(),
        paper_id: paper(&s.env),
        amount: PRICE,
        fee: 100_000,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.mecenas.address)]);
    assert_eq!(
        auths,
        std::vec![(
            reader.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.mecenas.address.clone(),
                    Symbol::new(&s.env, "take_read"),
                    (reader.clone(), publisher.clone(), paper(&s.env), PRICE).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
}

#[test]
fn una_por_lector_y_editorial_cada_24_horas() {
    let s = setup();
    let donor = s.funded(6 * PRICE);
    let publisher = Address::generate(&s.env);
    let other_publisher = Address::generate(&s.env);
    let ana = Address::generate(&s.env);
    let beto = Address::generate(&s.env);
    s.mecenas.leave_read(&donor, &publisher, &PRICE, &4);
    s.mecenas.leave_read(&donor, &other_publisher, &PRICE, &2);
    let take =
        |who: &Address, from: &Address| s.mecenas.try_take_read(who, from, &paper(&s.env), &PRICE);

    assert!(take(&ana, &publisher).is_ok());
    assert_eq!(take(&ana, &publisher), Err(Ok(Error::AlreadyTaken.into())));
    // Otra persona sí, y Ana sí con otra editorial.
    assert!(take(&beto, &publisher).is_ok());
    assert!(take(&ana, &other_publisher).is_ok());
    assert_eq!(s.mecenas.pending_reads(&publisher, &PRICE), 2);

    // Faltando poco para las 24 h todavía no; pasadas, sí.
    s.advance(DAY - 60);
    assert_eq!(take(&ana, &publisher), Err(Ok(Error::AlreadyTaken.into())));
    s.advance(120);
    assert!(take(&ana, &publisher).is_ok());
    assert_eq!(s.mecenas.pending_reads(&publisher, &PRICE), 1);
    assert_eq!(s.usdc.balance(&publisher), 3 * 4_900_000);
}

#[test]
fn sin_lecturas_pendientes_a_ese_precio_no_hay_nada_que_tomar() {
    let s = setup();
    let donor = s.funded(PRICE);
    let publisher = Address::generate(&s.env);
    let other_publisher = Address::generate(&s.env);
    let ana = Address::generate(&s.env);
    let beto = Address::generate(&s.env);
    s.mecenas.leave_read(&donor, &publisher, &PRICE, &1);

    // Ni a otro precio (no se toma más de lo que alguien dejó pagado) ni de otra editorial.
    assert_eq!(
        s.mecenas
            .try_take_read(&ana, &publisher, &paper(&s.env), &(PRICE + 1)),
        Err(Ok(Error::NoPendingRead.into()))
    );
    assert_eq!(
        s.mecenas
            .try_take_read(&ana, &other_publisher, &paper(&s.env), &PRICE),
        Err(Ok(Error::NoPendingRead.into()))
    );
    for id in ["", "x".repeat(65).as_str()] {
        assert_eq!(
            s.mecenas
                .try_take_read(&ana, &publisher, &String::from_str(&s.env, id), &PRICE),
            Err(Ok(Error::InvalidPaperId.into()))
        );
    }
    s.env.set_auths(&[]);
    assert!(s
        .mecenas
        .try_take_read(&ana, &publisher, &paper(&s.env), &PRICE)
        .is_err());
    s.env.mock_all_auths();
    assert_eq!(s.held(), PRICE);

    // La última se toma y la entrada desaparece; la siguiente persona ya no encuentra nada.
    s.mecenas
        .take_read(&ana, &publisher, &paper(&s.env), &PRICE);
    assert_eq!(s.mecenas.pending_reads(&publisher, &PRICE), 0);
    let stored = s.env.as_contract(&s.mecenas.address, || {
        s.env
            .storage()
            .persistent()
            .has(&DataKey::Pending(publisher.clone(), PRICE))
    });
    assert!(!stored);
    assert_eq!(
        s.mecenas
            .try_take_read(&beto, &publisher, &paper(&s.env), &PRICE),
        Err(Ok(Error::NoPendingRead.into()))
    );
    assert_eq!(s.held(), 0);
}

// ---------- TTL y custodia ----------

#[test]
fn el_flujo_y_las_lecturas_pendientes_viven_en_la_red_lo_que_hace_falta() {
    let s = setup();
    let (_, _, id) = s.opened();
    let donor = s.funded(2 * PRICE);
    let publisher = Address::generate(&s.env);
    s.mecenas.leave_read(&donor, &publisher, &PRICE, &2);
    let (left_ttl, left_max) = s.env.as_contract(&s.mecenas.address, || {
        (
            s.env
                .storage()
                .persistent()
                .get_ttl(&DataKey::Pending(publisher.clone(), PRICE)),
            s.env.storage().max_ttl(),
        )
    });
    assert_eq!(left_ttl, left_max, "recién dejadas ya viven el máximo");
    s.advance(30 * DAY);
    s.mecenas
        .take_read(&donor, &publisher, &paper(&s.env), &PRICE);

    let (stream_ttl, pending_ttl, max) = s.env.as_contract(&s.mecenas.address, || {
        let storage = s.env.storage().persistent();
        (
            storage.get_ttl(&DataKey::Stream(id)),
            storage.get_ttl(&DataKey::Pending(publisher.clone(), PRICE)),
            s.env.storage().max_ttl(),
        )
    });
    // El flujo: hasta su final, 30 días de gracia para retirar y una semana de margen (medido 30
    // días después de abrirlo).
    assert_eq!(
        stream_ttl,
        ((SPAN + 30 * DAY) / 5) as u32 + 7 * DAY_LEDGERS - (30 * DAY / 5) as u32
    );
    // Las lecturas pendientes no vencen: cada movimiento las lleva al máximo.
    assert_eq!(pending_ttl, max);
}

#[test]
fn el_saldo_del_contrato_es_lo_no_retirado_de_los_flujos_mas_las_lecturas_pendientes() {
    let s = setup();
    let (patron_a, lab_a, first) = s.opened();
    let (_, lab_b, second) = s.opened();
    let donor = s.funded(3 * PRICE);
    let publisher = Address::generate(&s.env);
    let reader = Address::generate(&s.env);
    let check = || {
        let mut held = s.mecenas.pending_reads(&publisher, &PRICE) as i128 * PRICE;
        for id in [first, second] {
            let stream = s.mecenas.stream(&id).unwrap();
            held += stream.amount - stream.withdrawn;
        }
        assert_eq!(s.held(), held);
    };
    check();

    s.mecenas.leave_read(&donor, &publisher, &PRICE, &3);
    check();
    s.advance(SPAN / 3);
    s.mecenas.withdraw_stream(&lab_a, &first);
    check();
    s.mecenas
        .take_read(&reader, &publisher, &paper(&s.env), &PRICE);
    check();
    s.advance(SPAN / 3);
    s.mecenas.cancel_stream(&patron_a, &first);
    check();
    s.advance(SPAN);
    s.mecenas.withdraw_stream(&lab_a, &first);
    s.mecenas.withdraw_stream(&lab_b, &second);
    check();
    assert_eq!(s.held(), 2 * PRICE, "quedan dos lecturas pendientes");
    assert_eq!(s.now(), START + SPAN / 3 * 2 + SPAN);
}
