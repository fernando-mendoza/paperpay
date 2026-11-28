//! Renta con opción a compra (v0.18.0): `rent` y `buy_out`.

use super::*;
use crate::{BoughtOut, Rental, Rented, MAX_RENT_SECONDS};

/// La renta de prueba: 0.10 USDC por 48 horas. Comprar cuesta `PRICE` (0.50).
const RENT: i128 = 1_000_000;
const TWO_DAYS: u64 = 2 * DAY;

fn other_paper(env: &Env) -> String {
    String::from_str(env, "quantum-cryptography-post-quantum")
}

/// `payer` renta el artículo de prueba por 48 horas. Devuelve hasta cuándo vale.
fn rented(s: &Setup) -> u64 {
    s.splitter.rent(
        &s.payer,
        &s.publisher,
        &paper(&s.env),
        &RENT,
        &PRICE,
        &TWO_DAYS,
    )
}

// ---------- rent ----------

#[test]
fn rentar_cobra_la_renta_con_el_reparto_98_2_y_guarda_el_precio_de_compra() {
    let s = setup();
    let before = s.usdc.balance(&s.payer);
    let until = rented(&s);
    let receipts = s.receipts();

    assert_eq!(until, START + TWO_DAYS);
    assert_eq!(s.usdc.balance(&s.payer), before - RENT);
    assert_eq!(s.usdc.balance(&s.publisher), 980_000);
    assert_eq!(s.usdc.balance(&s.treasury), 20_000);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        0,
        "el contrato no guarda fondos"
    );
    assert_eq!(
        s.splitter.rental(&s.payer, &s.publisher, &paper(&s.env)),
        Some(Rental {
            paid: RENT,
            full_price: PRICE,
            until,
        })
    );
    // Una renta no es una compra: emite `rented`, nunca `paid`.
    let expected = Rented {
        publisher: s.publisher.clone(),
        reader: s.payer.clone(),
        paper_id: paper(&s.env),
        amount: RENT,
        fee: 20_000,
        full_price: PRICE,
        until,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn rentar_solo_pide_la_firma_de_quien_lee_y_cubre_las_dos_transferencias() {
    let s = setup();
    rented(&s);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.payer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "rent"),
                    (
                        s.payer.clone(),
                        s.publisher.clone(),
                        paper(&s.env),
                        RENT,
                        PRICE,
                        TWO_DAYS
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![
                    s.transfer_auth(&s.payer, &s.publisher, 980_000),
                    s.transfer_auth(&s.payer, &s.treasury, 20_000),
                ],
            }
        )]
    );
}

#[test]
fn rentar_rechaza_montos_precios_periodos_e_ids_invalidos() {
    let s = setup();
    let before = s.usdc.balance(&s.payer);
    let rent = |amount: i128, full: i128, period: u64| {
        s.splitter.try_rent(
            &s.payer,
            &s.publisher,
            &paper(&s.env),
            &amount,
            &full,
            &period,
        )
    };

    for amount in [0, -1] {
        assert_eq!(
            rent(amount, PRICE, TWO_DAYS),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    // La renta tiene que costar menos que la compra.
    for full in [RENT, RENT - 1] {
        assert_eq!(
            rent(RENT, full, TWO_DAYS),
            Err(Ok(Error::RentNotCheaper.into()))
        );
    }
    for period in [0, MAX_RENT_SECONDS + 1] {
        assert_eq!(
            rent(RENT, PRICE, period),
            Err(Ok(Error::InvalidDays.into()))
        );
    }
    assert_eq!(
        s.splitter.try_rent(
            &s.payer,
            &s.publisher,
            &String::from_str(&s.env, ""),
            &RENT,
            &PRICE,
            &TWO_DAYS
        ),
        Err(Ok(Error::InvalidPaperId.into()))
    );
    s.env.set_auths(&[]);
    assert!(rent(RENT, PRICE, TWO_DAYS).is_err());
    assert_eq!(s.usdc.balance(&s.payer), before);
    assert_eq!(
        s.splitter.rental(&s.payer, &s.publisher, &paper(&s.env)),
        None
    );

    // Los bordes sí valen: un segundo, el máximo, y una compra un stroop más cara que la renta.
    s.env.mock_all_auths();
    assert!(rent(RENT, RENT + 1, 1).is_ok());
    assert!(s
        .splitter
        .try_rent(
            &s.payer,
            &s.publisher,
            &other_paper(&s.env),
            &RENT,
            &PRICE,
            &MAX_RENT_SECONDS
        )
        .is_ok());
}

#[test]
fn una_renta_vigente_no_se_repite_y_una_vencida_se_reemplaza() {
    let s = setup();
    let until = rented(&s);

    s.advance(TWO_DAYS - 1);
    assert_eq!(
        s.splitter.try_rent(
            &s.payer,
            &s.publisher,
            &paper(&s.env),
            &RENT,
            &PRICE,
            &TWO_DAYS
        ),
        Err(Ok(Error::RentalActive.into()))
    );
    assert_eq!(s.usdc.balance(&s.publisher), 980_000);

    // En el segundo en que vence ya se puede rentar otra vez, con otras condiciones.
    s.advance(1);
    let again = s.splitter.rent(
        &s.payer,
        &s.publisher,
        &paper(&s.env),
        &2_000_000,
        &4_000_000,
        &DAY,
    );
    assert_eq!(again, until + DAY);
    assert_eq!(
        s.splitter.rental(&s.payer, &s.publisher, &paper(&s.env)),
        Some(Rental {
            paid: 2_000_000,
            full_price: 4_000_000,
            until: again,
        })
    );
    assert_eq!(s.usdc.balance(&s.publisher), 980_000 + 1_960_000);
}

#[test]
fn la_renta_es_por_lector_editorial_y_articulo() {
    let s = setup();
    let other_reader = Address::generate(&s.env);
    let other_publisher = Address::generate(&s.env);
    s.fund(&other_reader, PRICE);
    rented(&s);

    // Otro artículo, otra editorial y otro lector no chocan con esa renta.
    s.splitter.rent(
        &s.payer,
        &s.publisher,
        &other_paper(&s.env),
        &RENT,
        &PRICE,
        &TWO_DAYS,
    );
    s.splitter.rent(
        &s.payer,
        &other_publisher,
        &paper(&s.env),
        &RENT,
        &PRICE,
        &TWO_DAYS,
    );
    s.splitter.rent(
        &other_reader,
        &s.publisher,
        &paper(&s.env),
        &RENT,
        &PRICE,
        &DAY,
    );
    assert_eq!(
        s.splitter
            .rental(&other_reader, &s.publisher, &paper(&s.env))
            .unwrap()
            .until,
        START + DAY
    );
    assert_eq!(
        s.splitter
            .rental(&s.payer, &s.publisher, &paper(&s.env))
            .unwrap()
            .until,
        START + TWO_DAYS
    );
    assert_eq!(s.usdc.balance(&s.publisher), 3 * 980_000);
    assert_eq!(s.usdc.balance(&other_publisher), 980_000);
}

#[test]
fn sin_saldo_no_hay_renta() {
    let s = setup();
    let broke = Address::generate(&s.env);
    assert!(s
        .splitter
        .try_rent(
            &broke,
            &s.publisher,
            &paper(&s.env),
            &RENT,
            &PRICE,
            &TWO_DAYS
        )
        .is_err());
    assert_eq!(
        s.splitter.rental(&broke, &s.publisher, &paper(&s.env)),
        None
    );
    assert_eq!(s.usdc.balance(&s.publisher), 0);
}

// ---------- buy_out ----------

#[test]
fn comprar_despues_de_rentar_cuesta_solo_la_diferencia() {
    let s = setup();
    let before = s.usdc.balance(&s.payer);
    rented(&s);
    let split = s.splitter.buy_out(&s.payer, &s.publisher, &paper(&s.env));
    let receipts = s.receipts();

    assert_eq!(
        split,
        Split {
            publisher: 3_920_000,
            fee: 80_000
        }
    );
    // Rentar y después comprar le cuesta al lector, y le deja a la editorial y a la tesorería,
    // exactamente lo mismo que comprar de entrada.
    assert_eq!(s.usdc.balance(&s.payer), before - PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(
        s.splitter.rental(&s.payer, &s.publisher, &paper(&s.env)),
        None
    );
    assert_eq!(
        receipts,
        [
            Paid {
                publisher: s.publisher.clone(),
                reader: s.payer.clone(),
                paper_id: paper(&s.env),
                payer: s.payer.clone(),
                amount: PRICE - RENT,
                fee: 80_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            BoughtOut {
                publisher: s.publisher.clone(),
                reader: s.payer.clone(),
                paper_id: paper(&s.env),
                rented: RENT,
                amount: PRICE - RENT,
                total: PRICE,
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );
}

#[test]
fn comprar_solo_pide_la_firma_de_quien_lee_y_cubre_la_diferencia() {
    let s = setup();
    rented(&s);
    s.splitter.buy_out(&s.payer, &s.publisher, &paper(&s.env));

    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.payer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "buy_out"),
                    (s.payer.clone(), s.publisher.clone(), paper(&s.env)).into_val(&s.env),
                )),
                sub_invocations: std::vec![
                    s.transfer_auth(&s.payer, &s.publisher, 3_920_000),
                    s.transfer_auth(&s.payer, &s.treasury, 80_000),
                ],
            }
        )]
    );
}

#[test]
fn la_compra_con_descuento_solo_vale_mientras_dura_la_renta() {
    let s = setup();
    rented(&s);
    s.splitter.rent(
        &s.payer,
        &s.publisher,
        &other_paper(&s.env),
        &RENT,
        &PRICE,
        &TWO_DAYS,
    );

    s.advance(TWO_DAYS - 1);
    assert!(s
        .splitter
        .try_buy_out(&s.payer, &s.publisher, &paper(&s.env))
        .is_ok());

    s.advance(1);
    assert_eq!(
        s.splitter
            .try_buy_out(&s.payer, &s.publisher, &other_paper(&s.env)),
        Err(Ok(Error::RentalExpired.into()))
    );
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000 + 980_000);
    // La renta vencida sigue anotada (se puede volver a rentar), pero ya no da descuento.
    assert!(s
        .splitter
        .rental(&s.payer, &s.publisher, &other_paper(&s.env))
        .is_some());
}

#[test]
fn no_se_compra_sin_renta_ni_dos_veces_ni_con_la_renta_de_otro() {
    let s = setup();
    let other_reader = Address::generate(&s.env);
    s.fund(&other_reader, PRICE);

    assert_eq!(
        s.splitter
            .try_buy_out(&s.payer, &s.publisher, &paper(&s.env)),
        Err(Ok(Error::NoRental.into()))
    );
    rented(&s);
    // La renta de una cuenta no le da descuento a otra, ni en otro artículo.
    assert_eq!(
        s.splitter
            .try_buy_out(&other_reader, &s.publisher, &paper(&s.env)),
        Err(Ok(Error::NoRental.into()))
    );
    assert_eq!(
        s.splitter
            .try_buy_out(&s.payer, &s.publisher, &other_paper(&s.env)),
        Err(Ok(Error::NoRental.into()))
    );
    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_buy_out(&s.payer, &s.publisher, &paper(&s.env))
        .is_err());
    assert_eq!(s.usdc.balance(&s.publisher), 980_000);

    s.env.mock_all_auths();
    s.splitter.buy_out(&s.payer, &s.publisher, &paper(&s.env));
    assert_eq!(
        s.splitter
            .try_buy_out(&s.payer, &s.publisher, &paper(&s.env)),
        Err(Ok(Error::NoRental.into()))
    );
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&other_reader), PRICE);
}

#[test]
fn sin_saldo_para_la_diferencia_la_renta_sigue_y_no_se_mueve_nada() {
    let s = setup();
    let reader = Address::generate(&s.env);
    s.fund(&reader, RENT);
    s.splitter.rent(
        &reader,
        &s.publisher,
        &paper(&s.env),
        &RENT,
        &PRICE,
        &TWO_DAYS,
    );

    assert!(s
        .splitter
        .try_buy_out(&reader, &s.publisher, &paper(&s.env))
        .is_err());
    assert!(s
        .splitter
        .rental(&reader, &s.publisher, &paper(&s.env))
        .is_some());
    assert_eq!(s.usdc.balance(&s.publisher), 980_000);

    // Con saldo, la misma renta todavía se convierte.
    s.fund(&reader, PRICE - RENT);
    s.splitter.buy_out(&reader, &s.publisher, &paper(&s.env));
    assert_eq!(s.usdc.balance(&reader), 0);
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
}

#[test]
fn la_comision_se_redondea_en_cada_pago_a_favor_de_la_editorial() {
    let s = setup();
    // Renta de 1 000 049 (comisión 20 000) y compra de 5 000 001: la diferencia es 3 999 952
    // (comisión 79 999).
    s.splitter.rent(
        &s.payer,
        &s.publisher,
        &paper(&s.env),
        &1_000_049,
        &5_000_001,
        &TWO_DAYS,
    );
    let split = s.splitter.buy_out(&s.payer, &s.publisher, &paper(&s.env));

    assert_eq!(split.fee, 79_999);
    assert_eq!(split.publisher, 3_919_953);
    assert_eq!(s.usdc.balance(&s.treasury), 20_000 + 79_999);
    assert_eq!(s.usdc.balance(&s.publisher), 980_049 + 3_919_953);
    assert_eq!(
        s.usdc.balance(&s.publisher) + s.usdc.balance(&s.treasury),
        5_000_001,
        "entre los dos pagos, justo el precio de compra"
    );
}

// ---------- TTL ----------

#[test]
fn la_renta_vive_en_la_red_hasta_que_vence() {
    let s = setup();
    rented(&s);
    let ttl = s.env.as_contract(&s.splitter.address, || {
        s.env.storage().persistent().get_ttl(&DataKey::Rental(
            s.payer.clone(),
            s.publisher.clone(),
            paper(&s.env),
        ))
    });
    assert_eq!(ttl, (TWO_DAYS / 5) as u32 + 7 * DAY_LEDGERS);
}

// ---------- extend_rental ----------

#[test]
fn extender_la_renta_suma_tiempo_y_lo_pagado_cuenta_para_la_compra() {
    let s = setup();
    let before = s.usdc.balance(&s.payer);
    let until = rented(&s);
    let extended = s
        .splitter
        .extend_rental(&s.payer, &s.publisher, &paper(&s.env), &RENT, &DAY);
    let receipts = s.receipts();
    let auths = s.env.auths();

    assert_eq!(extended, until + DAY);
    assert_eq!(
        s.splitter.rental(&s.payer, &s.publisher, &paper(&s.env)),
        Some(Rental {
            paid: 2 * RENT,
            full_price: PRICE,
            until: extended,
        })
    );
    assert_eq!(s.usdc.balance(&s.publisher), 2 * 980_000);
    assert_eq!(s.usdc.balance(&s.treasury), 2 * 20_000);
    let expected = Rented {
        publisher: s.publisher.clone(),
        reader: s.payer.clone(),
        paper_id: paper(&s.env),
        amount: RENT,
        fee: 20_000,
        full_price: PRICE,
        until: extended,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
    assert_eq!(
        auths,
        std::vec![(
            s.payer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "extend_rental"),
                    (
                        s.payer.clone(),
                        s.publisher.clone(),
                        paper(&s.env),
                        RENT,
                        DAY
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![
                    s.transfer_auth(&s.payer, &s.publisher, 980_000),
                    s.transfer_auth(&s.payer, &s.treasury, 20_000),
                ],
            }
        )]
    );

    // Con 0.20 ya pagados, comprar cuesta solo los 0.30 que faltan.
    let split = s.splitter.buy_out(&s.payer, &s.publisher, &paper(&s.env));
    assert_eq!(split.publisher + split.fee, PRICE - 2 * RENT);
    assert_eq!(s.usdc.balance(&s.payer), before - PRICE);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

#[test]
fn extender_exige_una_renta_vigente_y_que_siga_siendo_mas_barata_que_comprar() {
    let s = setup();
    let extend = |amount: i128, period: u64| {
        s.splitter
            .try_extend_rental(&s.payer, &s.publisher, &paper(&s.env), &amount, &period)
    };

    assert_eq!(extend(RENT, DAY), Err(Ok(Error::NoRental.into())));
    rented(&s);
    for amount in [0, -1] {
        assert_eq!(extend(amount, DAY), Err(Ok(Error::InvalidAmount.into())));
    }
    // Con 0.10 pagados y compra de 0.50, extender por 0.40 ya sería pagar el precio completo.
    assert_eq!(
        extend(PRICE - RENT, DAY),
        Err(Ok(Error::RentNotCheaper.into()))
    );
    // Quedan 2 días de renta y el vencimiento no pasa de 7 desde ahora: 5 días más sí, un
    // segundo más no.
    for period in [0, MAX_RENT_SECONDS + 1, 5 * DAY + 1] {
        assert_eq!(extend(RENT, period), Err(Ok(Error::InvalidDays.into())));
    }
    s.env.set_auths(&[]);
    assert!(extend(RENT, DAY).is_err());
    s.env.mock_all_auths();
    assert_eq!(s.usdc.balance(&s.publisher), 980_000);

    // Los dos bordes a la vez: un stroop menos que el precio y justo 7 días.
    assert_eq!(extend(PRICE - RENT - 1, 5 * DAY), Ok(Ok(START + 7 * DAY)));

    // Vencida, ya no se extiende: hay que rentar de nuevo.
    s.advance(7 * DAY);
    assert_eq!(extend(1, 1), Err(Ok(Error::RentalExpired.into())));
}
