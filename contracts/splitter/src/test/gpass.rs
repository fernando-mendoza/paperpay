//! Pase PaperPay (v0.11.0): `buy_global_pass`, `use_pass` (soporte), `settle_pass` y `cancel_pass`.

use super::*;
use crate::{
    GlobalPass, GlobalPassBought, PassCancelled, PassRead, PassSettled, PassShare,
    MAX_TAB_PUBLISHERS,
};
use soroban_sdk::Map;

/// Lo que cuesta el pase de prueba: 3 USDC por 7 días.
const PASS: i128 = 30_000_000;

struct Holder {
    reader: Address,
    operator: Address,
    until: u64,
}

impl Setup<'_> {
    /// Un Pase PaperPay de 3 USDC por 7 días.
    fn bought_pass(&self) -> Holder {
        self.bought_pass_of(PASS)
    }

    fn bought_pass_of(&self, amount: i128) -> Holder {
        let reader = Address::generate(&self.env);
        let operator = Address::generate(&self.env);
        self.fund(&reader, amount);
        let until = self
            .splitter
            .buy_global_pass(&reader, &operator, &amount, &(7 * DAY));
        Holder {
            reader,
            operator,
            until,
        }
    }

    /// El operador anota `times` lecturas de `publisher` en el pase de `holder`.
    fn read_with_pass(&self, holder: &Holder, publisher: &Address, times: u32) -> u32 {
        let mut reads = 0;
        for _ in 0..times {
            reads = self.splitter.use_pass(
                &holder.operator,
                &holder.reader,
                publisher,
                &paper(&self.env),
            );
        }
        reads
    }
}

// ---------- buy_global_pass ----------

#[test]
fn el_pase_queda_en_garantia_hasta_que_vence() {
    let s = setup();
    let reader = Address::generate(&s.env);
    let operator = Address::generate(&s.env);
    s.fund(&reader, PASS + PRICE);
    let until = s
        .splitter
        .buy_global_pass(&reader, &operator, &PASS, &(7 * DAY));
    let receipts = s.receipts();

    assert_eq!(until, s.now() + 7 * DAY);
    assert_eq!(s.usdc.balance(&reader), PRICE);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        PASS,
        "lo pagado queda en el contrato"
    );
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(
        s.splitter.global_pass(&reader),
        Some(GlobalPass {
            operator: operator.clone(),
            amount: PASS,
            until,
            reads: 0,
            by_publisher: Map::new(&s.env),
        })
    );
    let expected = GlobalPassBought {
        reader: reader.clone(),
        operator,
        amount: PASS,
        until,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn comprar_el_pase_solo_pide_la_firma_de_quien_lee() {
    let s = setup();
    let reader = Address::generate(&s.env);
    let operator = Address::generate(&s.env);
    s.fund(&reader, PASS);
    s.splitter
        .buy_global_pass(&reader, &operator, &PASS, &(7 * DAY));

    assert_eq!(
        s.env.auths(),
        std::vec![(
            reader.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "buy_global_pass"),
                    (reader.clone(), operator.clone(), PASS, 7 * DAY).into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(&reader, &s.splitter.address, PASS)],
            }
        )]
    );
}

#[test]
fn comprar_rechaza_monto_y_duracion_invalidos_un_segundo_pase_y_sin_firma_no_hay_pase() {
    let s = setup();
    let reader = Address::generate(&s.env);
    let operator = Address::generate(&s.env);
    s.fund(&reader, 3 * PASS);
    let max = MAX_PASS_DAYS as u64 * DAY;

    for amount in [0, -1] {
        assert_eq!(
            s.splitter
                .try_buy_global_pass(&reader, &operator, &amount, &DAY),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    for period in [0, max + 1] {
        assert_eq!(
            s.splitter
                .try_buy_global_pass(&reader, &operator, &PASS, &period),
            Err(Ok(Error::InvalidDays.into()))
        );
    }
    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_buy_global_pass(&reader, &operator, &PASS, &DAY)
        .is_err());
    assert_eq!(s.splitter.global_pass(&reader), None);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);

    // Los bordes de la duración valen (1 segundo y 31 días), y un pase a la vez.
    s.env.mock_all_auths();
    s.splitter.buy_global_pass(&reader, &operator, &PASS, &max);
    let other = Address::generate(&s.env);
    s.fund(&other, PASS);
    s.splitter.buy_global_pass(&other, &operator, &PASS, &1);
    assert_eq!(
        s.splitter
            .try_buy_global_pass(&reader, &operator, &PASS, &DAY),
        Err(Ok(Error::PassExists.into()))
    );
    assert_eq!(s.usdc.balance(&reader), 2 * PASS);
}

// ---------- use_pass ----------

#[test]
fn el_operador_anota_las_lecturas_del_pase_por_editorial_sin_mover_fondos() {
    let s = setup();
    let holder = s.bought_pass();
    let other_publisher = Address::generate(&s.env);

    let reads = s.splitter.use_pass(
        &holder.operator,
        &holder.reader,
        &s.publisher,
        &paper(&s.env),
    );
    let receipts = s.receipts();
    assert_eq!(reads, 1);
    let expected = PassRead {
        reader: holder.reader.clone(),
        publisher: s.publisher.clone(),
        paper_id: paper(&s.env),
        reads: 1,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    assert_eq!(s.read_with_pass(&holder, &other_publisher, 1), 2);
    assert_eq!(s.read_with_pass(&holder, &s.publisher, 2), 4);
    let pass = s.splitter.global_pass(&holder.reader).unwrap();
    assert_eq!(pass.reads, 4);
    assert_eq!(pass.by_publisher.get(s.publisher.clone()), Some(3));
    assert_eq!(pass.by_publisher.get(other_publisher.clone()), Some(1));
    assert_eq!(pass.amount, PASS);
    assert_eq!(s.usdc.balance(&s.splitter.address), PASS, "nada se movió");
    assert_eq!(s.usdc.balance(&s.publisher), 0);
}

#[test]
fn solo_el_operador_del_pase_anota_lecturas_y_solo_mientras_esta_vigente() {
    let s = setup();
    let holder = s.bought_pass();
    let stranger = Address::generate(&s.env);
    let use_pass = |operator: &Address, reader: &Address, paper_id: &String| {
        s.splitter
            .try_use_pass(operator, reader, &s.publisher, paper_id)
    };

    s.read_with_pass(&holder, &s.publisher, 1);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            holder.operator.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "use_pass"),
                    (
                        holder.operator.clone(),
                        holder.reader.clone(),
                        s.publisher.clone(),
                        paper(&s.env)
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );

    for signer in [&stranger, &holder.reader, &s.publisher] {
        assert_eq!(
            use_pass(signer, &holder.reader, &paper(&s.env)),
            Err(Ok(Error::NotOperator.into()))
        );
    }
    assert_eq!(
        use_pass(&holder.operator, &stranger, &paper(&s.env)),
        Err(Ok(Error::NoGlobalPass.into()))
    );
    assert_eq!(
        use_pass(
            &holder.operator,
            &holder.reader,
            &String::from_str(&s.env, "")
        ),
        Err(Ok(Error::InvalidPaperId.into()))
    );
    s.env.set_auths(&[]);
    assert!(use_pass(&holder.operator, &holder.reader, &paper(&s.env)).is_err());
    s.env.mock_all_auths();

    // Un segundo antes de vencer todavía cuenta; en el segundo del vencimiento, ya no.
    s.advance(7 * DAY - 5);
    s.env.ledger().with_mut(|li| li.timestamp += 4);
    assert_eq!(s.now(), holder.until - 1);
    assert_eq!(
        use_pass(&holder.operator, &holder.reader, &paper(&s.env)),
        Ok(Ok(2))
    );
    s.env.ledger().with_mut(|li| li.timestamp += 1);
    assert_eq!(
        use_pass(&holder.operator, &holder.reader, &paper(&s.env)),
        Err(Ok(Error::PassExpired.into()))
    );
    assert_eq!(s.splitter.global_pass(&holder.reader).unwrap().reads, 2);
}

#[test]
fn un_pase_cuenta_lecturas_de_hasta_diez_editoriales() {
    let s = setup();
    let holder = s.bought_pass();
    let mut publishers = std::vec::Vec::new();
    for _ in 0..MAX_TAB_PUBLISHERS {
        let publisher = Address::generate(&s.env);
        s.read_with_pass(&holder, &publisher, 1);
        publishers.push(publisher);
    }
    assert_eq!(
        s.splitter.try_use_pass(
            &holder.operator,
            &holder.reader,
            &Address::generate(&s.env),
            &paper(&s.env)
        ),
        Err(Ok(Error::PassFull.into()))
    );
    // Más lecturas de una de las diez sí.
    assert_eq!(
        s.read_with_pass(&holder, &publishers[3], 1),
        MAX_TAB_PUBLISHERS + 1
    );
}

// ---------- settle_pass ----------

#[test]
fn vencido_el_pase_se_reparte_entre_las_editoriales_segun_las_lecturas() {
    let s = setup();
    let holder = s.bought_pass();
    let other_publisher = Address::generate(&s.env);
    s.read_with_pass(&holder, &s.publisher, 3);
    s.read_with_pass(&holder, &other_publisher, 1);
    s.advance(7 * DAY);

    // Sin ninguna firma.
    s.env.set_auths(&[]);
    let split = s.splitter.settle_pass(&holder.reader);
    let receipts = s.receipts();
    assert_eq!(s.env.auths(), std::vec![]);

    // 2% de 3.00 a la tesorería; de los 2.94 restantes, tres cuartos y un cuarto.
    assert_eq!(
        split,
        Split {
            publisher: 29_400_000,
            fee: 600_000
        }
    );
    assert_eq!(s.usdc.balance(&s.publisher), 22_050_000);
    assert_eq!(s.usdc.balance(&other_publisher), 7_350_000);
    assert_eq!(s.usdc.balance(&s.treasury), 600_000);
    assert_eq!(s.usdc.balance(&holder.reader), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.splitter.global_pass(&holder.reader), None);

    // Un `pass_share` por editorial (en el orden del mapa) y el `pass_settled`.
    let share = |publisher: &Address, reads: u32, amount: i128| {
        PassShare {
            publisher: publisher.clone(),
            reader: holder.reader.clone(),
            reads,
            amount,
        }
        .to_xdr(&s.env, &s.splitter.address)
    };
    let settled = PassSettled {
        reader: holder.reader.clone(),
        amount: PASS,
        fee: 600_000,
        reads: 4,
        publishers: 2,
    }
    .to_xdr(&s.env, &s.splitter.address);
    let first_then_other = [
        share(&s.publisher, 3, 22_050_000),
        share(&other_publisher, 1, 7_350_000),
        settled.clone(),
    ];
    let other_then_first = [
        share(&other_publisher, 1, 7_350_000),
        share(&s.publisher, 3, 22_050_000),
        settled,
    ];
    assert!(receipts == first_then_other || receipts == other_then_first);
}

#[test]
fn lo_que_sobra_por_redondeo_va_a_una_editorial_y_no_se_pierde_nada() {
    let s = setup();
    // 1 000 003: comisión 20 000 (piso), quedan 980 003 para tres editoriales con una lectura.
    let holder = s.bought_pass_of(1_000_003);
    let publishers = [
        Address::generate(&s.env),
        Address::generate(&s.env),
        Address::generate(&s.env),
    ];
    for publisher in &publishers {
        s.read_with_pass(&holder, publisher, 1);
    }
    s.advance(7 * DAY);
    let split = s.splitter.settle_pass(&holder.reader);

    assert_eq!(split.fee, 20_000);
    assert_eq!(split.publisher, 980_003);
    let mut got: std::vec::Vec<i128> = publishers.iter().map(|p| s.usdc.balance(p)).collect();
    got.sort();
    // Piso de 980 003 / 3 = 326 667 cada una; los 2 que sobran van a una sola.
    assert_eq!(got, [326_667, 326_667, 326_669]);
    assert_eq!(s.usdc.balance(&s.treasury), 20_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

#[test]
fn un_pase_no_se_liquida_antes_de_vencer_ni_dos_veces() {
    let s = setup();
    let holder = s.bought_pass();
    let stranger = Address::generate(&s.env);
    s.read_with_pass(&holder, &s.publisher, 1);

    assert_eq!(
        s.splitter.try_settle_pass(&stranger),
        Err(Ok(Error::NoGlobalPass.into()))
    );
    // Un segundo antes de vencer no; en el vencimiento, sí.
    s.advance(7 * DAY - 5);
    s.env.ledger().with_mut(|li| li.timestamp += 4);
    assert_eq!(
        s.splitter.try_settle_pass(&holder.reader),
        Err(Ok(Error::PassActive.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), PASS);
    s.env.ledger().with_mut(|li| li.timestamp += 1);
    assert!(s.splitter.try_settle_pass(&holder.reader).is_ok());
    assert_eq!(
        s.splitter.try_settle_pass(&holder.reader),
        Err(Ok(Error::NoGlobalPass.into()))
    );
    assert_eq!(s.usdc.balance(&s.publisher), 29_400_000);

    // Liquidado, puede comprar otro.
    s.fund(&holder.reader, PASS);
    s.splitter
        .buy_global_pass(&holder.reader, &holder.operator, &PASS, &DAY);
    assert_eq!(s.splitter.global_pass(&holder.reader).unwrap().reads, 0);
}

#[test]
fn un_pase_vencido_sin_lecturas_vuelve_completo_a_quien_lo_pago() {
    let s = setup();
    let holder = s.bought_pass();
    s.advance(7 * DAY);
    let split = s.splitter.settle_pass(&holder.reader);
    let receipts = s.receipts();

    assert_eq!(
        split,
        Split {
            publisher: 0,
            fee: 0
        }
    );
    assert_eq!(s.usdc.balance(&holder.reader), PASS);
    assert_eq!(
        s.usdc.balance(&s.treasury),
        0,
        "sin lecturas no hay comisión"
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    let expected = PassSettled {
        reader: holder.reader.clone(),
        amount: PASS,
        fee: 0,
        reads: 0,
        publishers: 0,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn si_una_editorial_no_puede_recibir_su_parte_del_pase_queda_pendiente() {
    let s = setup();
    let holder = s.bought_pass();
    let nueva = g_account(&s.env, 61);
    s.read_with_pass(&holder, &nueva, 1);
    s.read_with_pass(&holder, &s.publisher, 1);
    s.advance(7 * DAY);
    s.splitter.settle_pass(&holder.reader);

    assert_eq!(s.usdc.balance(&s.publisher), 14_700_000);
    assert_eq!(s.splitter.owed(&nueva), 14_700_000);
    assert_eq!(s.usdc.balance(&s.treasury), 600_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 14_700_000);
}

// ---------- cancel_pass ----------

#[test]
fn sin_lecturas_quien_compro_el_pase_lo_cancela_y_recupera_todo() {
    let s = setup();
    let holder = s.bought_pass();
    let back = s.splitter.cancel_pass(&holder.reader);
    let receipts = s.receipts();

    assert_eq!(back, PASS);
    assert_eq!(s.usdc.balance(&holder.reader), PASS);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(s.splitter.global_pass(&holder.reader), None);
    let expected = PassCancelled {
        reader: holder.reader.clone(),
        amount: PASS,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    // Cancelado, el operador ya no anota nada y se puede comprar otro.
    assert_eq!(
        s.splitter.try_use_pass(
            &holder.operator,
            &holder.reader,
            &s.publisher,
            &paper(&s.env)
        ),
        Err(Ok(Error::NoGlobalPass.into()))
    );
    s.splitter
        .buy_global_pass(&holder.reader, &holder.operator, &PASS, &DAY);
}

#[test]
fn con_una_lectura_el_pase_ya_no_se_cancela_y_solo_lo_cancela_quien_lo_compro() {
    let s = setup();
    let holder = s.bought_pass();
    let unused = s.bought_pass();

    assert_eq!(
        s.splitter.try_cancel_pass(&holder.operator),
        Err(Ok(Error::NoGlobalPass.into()))
    );
    s.read_with_pass(&holder, &s.publisher, 1);
    assert_eq!(
        s.splitter.try_cancel_pass(&holder.reader),
        Err(Ok(Error::PassUsed.into()))
    );
    assert_eq!(s.usdc.balance(&holder.reader), 0);

    s.env.set_auths(&[]);
    assert!(s.splitter.try_cancel_pass(&unused.reader).is_err());
    assert_eq!(s.usdc.balance(&s.splitter.address), 2 * PASS);

    s.env.mock_all_auths();
    s.splitter.cancel_pass(&unused.reader);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            unused.reader.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "cancel_pass"),
                    (unused.reader.clone(),).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
    assert_eq!(s.usdc.balance(&unused.reader), PASS);
}

// ---------- TTL y custodia ----------

#[test]
fn el_pase_vive_en_la_red_hasta_que_vence_y_la_gracia_para_liquidarlo() {
    let s = setup();
    let holder = s.bought_pass();
    let key = DataKey::GlobalPass(holder.reader.clone());
    let ttl = || {
        s.env.as_contract(&s.splitter.address, || {
            s.env.storage().persistent().get_ttl(&key)
        })
    };
    let needed = ((7 + REFUND_GRACE_DAYS) as u64 * DAY / 5) as u32 + 7 * DAY_LEDGERS;
    assert_eq!(ttl(), needed);

    // Tres días después, anotar una lectura no la acorta ni la alarga: vive hasta la misma fecha.
    s.advance(3 * DAY);
    s.read_with_pass(&holder, &s.publisher, 1);
    assert_eq!(ttl(), needed - 3 * DAY_LEDGERS);
}

#[test]
fn el_saldo_del_contrato_cuenta_los_pases_sin_liquidar_y_lo_pendiente() {
    let s = setup();
    let ana = s.bought_pass();
    let beto = s.bought_pass();
    let caro = s.bought_pass();
    let nueva = g_account(&s.env, 63);
    let check = || {
        let mut held = s.splitter.owed(&nueva);
        for holder in [&ana, &beto, &caro] {
            if let Some(pass) = s.splitter.global_pass(&holder.reader) {
                held += pass.amount;
            }
        }
        assert_eq!(s.usdc.balance(&s.splitter.address), held);
    };
    check();

    s.read_with_pass(&ana, &s.publisher, 2);
    s.read_with_pass(&ana, &nueva, 1);
    s.splitter.cancel_pass(&caro.reader);
    check();

    s.advance(7 * DAY);
    s.splitter.settle_pass(&ana.reader);
    check();
    assert_eq!(s.splitter.owed(&nueva), 9_800_000);
    s.splitter.settle_pass(&beto.reader);
    check();
    s.splitter.claim(&nueva);
    check();
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.usdc.balance(&s.publisher), 19_600_000);
    assert_eq!(
        s.usdc.balance(&beto.reader),
        PASS,
        "no leyó nada: volvió todo"
    );
    assert_eq!(s.usdc.balance(&caro.reader), PASS);
    assert_eq!(s.usdc.balance(&s.treasury), 600_000);
}
