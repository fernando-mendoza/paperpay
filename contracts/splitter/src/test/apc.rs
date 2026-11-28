//! Cargo de publicación (APC) en garantía (v0.19.0): `pay_apc`, `accept_manuscript`,
//! `reject_manuscript` y `reclaim_apc`.

use super::*;
use crate::{Apc, ApcPaid, ApcReclaimed, ManuscriptAccepted, ManuscriptRejected, MAX_APC_DAYS};

/// El cargo de publicación de prueba: 2 USDC.
const CHARGE: i128 = 20_000_000;

fn manuscript(env: &Env) -> String {
    String::from_str(env, "ms-2026-0417")
}

/// Un manuscrito enviado con su cargo de 2 USDC en garantía y 30 días para decidir. Devuelve
/// quién lo envió, la revista y el id del cargo.
fn submitted(s: &Setup) -> (Address, Address, u32) {
    let author = Address::generate(&s.env);
    s.fund(&author, CHARGE);
    let journal = Address::generate(&s.env);
    let id = s.splitter.pay_apc(
        &author,
        &journal,
        &manuscript(&s.env),
        &CHARGE,
        &(s.now() + 30 * DAY),
    );
    (author, journal, id)
}

// ---------- pay_apc ----------

#[test]
fn el_cargo_de_publicacion_queda_en_el_contrato_hasta_que_la_revista_decida() {
    let s = setup();
    let author = Address::generate(&s.env);
    s.fund(&author, 2 * CHARGE);
    let journal = Address::generate(&s.env);
    let deadline = s.now() + 30 * DAY;
    let id = s
        .splitter
        .pay_apc(&author, &journal, &manuscript(&s.env), &CHARGE, &deadline);
    let receipts = s.receipts();

    assert_eq!(id, 0);
    assert_eq!(s.usdc.balance(&author), CHARGE);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        CHARGE,
        "el cargo queda en el contrato"
    );
    assert_eq!(s.usdc.balance(&journal), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(
        s.splitter.apc(&id),
        Some(Apc {
            author: author.clone(),
            journal: journal.clone(),
            manuscript_id: manuscript(&s.env),
            amount: CHARGE,
            deadline,
            open: true,
        })
    );
    let expected = ApcPaid {
        author: author.clone(),
        journal: journal.clone(),
        apc_id: 0,
        manuscript_id: manuscript(&s.env),
        amount: CHARGE,
        deadline,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    // Los ids son secuenciales y cada cargo guarda lo suyo.
    let second = s.splitter.pay_apc(
        &author,
        &journal,
        &manuscript(&s.env),
        &4_000_000,
        &(deadline + DAY),
    );
    assert_eq!(second, 1);
    assert_eq!(s.splitter.apc(&second).unwrap().amount, 4_000_000);
    assert_eq!(s.splitter.apc(&id).unwrap().amount, CHARGE);
    assert_eq!(s.splitter.apc(&2), None);
    assert_eq!(s.usdc.balance(&s.splitter.address), CHARGE + 4_000_000);
}

#[test]
fn pagar_el_cargo_solo_pide_la_firma_de_quien_envia() {
    let s = setup();
    let author = Address::generate(&s.env);
    s.fund(&author, CHARGE);
    let journal = Address::generate(&s.env);
    let deadline = s.now() + 30 * DAY;
    s.splitter
        .pay_apc(&author, &journal, &manuscript(&s.env), &CHARGE, &deadline);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            author.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "pay_apc"),
                    (
                        author.clone(),
                        journal.clone(),
                        manuscript(&s.env),
                        CHARGE,
                        deadline
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(&author, &s.splitter.address, CHARGE)],
            }
        )]
    );
}

#[test]
fn pagar_el_cargo_rechaza_monto_plazo_e_id_invalidos_y_sin_firma_no_hay_cargo() {
    let s = setup();
    let author = Address::generate(&s.env);
    s.fund(&author, 3 * CHARGE);
    let journal = Address::generate(&s.env);
    let now = s.now();
    let max = MAX_APC_DAYS as u64 * DAY;
    let pay = |amount: i128, deadline: u64| {
        s.splitter
            .try_pay_apc(&author, &journal, &manuscript(&s.env), &amount, &deadline)
    };

    for amount in [0, -1] {
        assert_eq!(pay(amount, now + DAY), Err(Ok(Error::InvalidAmount.into())));
    }
    for deadline in [now, now - 1, now + max + 1] {
        assert_eq!(
            pay(CHARGE, deadline),
            Err(Ok(Error::InvalidDeadline.into()))
        );
    }
    assert_eq!(
        s.splitter.try_pay_apc(
            &author,
            &journal,
            &String::from_str(&s.env, ""),
            &CHARGE,
            &(now + DAY)
        ),
        Err(Ok(Error::InvalidPaperId.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.splitter.apc(&0), None);

    // Los bordes del plazo sí valen: un segundo después de ahora y justo el máximo.
    assert!(pay(CHARGE, now + 1).is_ok());
    assert!(pay(CHARGE, now + max).is_ok());

    s.env.set_auths(&[]);
    assert!(pay(CHARGE, now + DAY).is_err());
    assert_eq!(s.usdc.balance(&author), CHARGE);
    assert_eq!(s.splitter.apc(&2), None);
}

// ---------- accept_manuscript ----------

#[test]
fn si_la_revista_acepta_cobra_el_98_y_la_tesoreria_el_2() {
    let s = setup();
    let (author, journal, id) = submitted(&s);
    let split = s.splitter.accept_manuscript(&journal, &id);
    let receipts = s.receipts();

    assert_eq!(
        split,
        Split {
            publisher: 19_600_000,
            fee: 400_000
        }
    );
    assert_eq!(s.usdc.balance(&journal), 19_600_000);
    assert_eq!(s.usdc.balance(&s.treasury), 400_000);
    assert_eq!(s.usdc.balance(&author), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert!(!s.splitter.apc(&id).unwrap().open);
    let expected = ManuscriptAccepted {
        journal: journal.clone(),
        author: author.clone(),
        apc_id: id,
        manuscript_id: manuscript(&s.env),
        amount: CHARGE,
        fee: 400_000,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn aceptar_solo_pide_la_firma_de_la_revista() {
    let s = setup();
    let (_, journal, id) = submitted(&s);
    s.splitter.accept_manuscript(&journal, &id);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            journal.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "accept_manuscript"),
                    (journal.clone(), id).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
}

#[test]
fn solo_la_revista_del_cargo_acepta_o_rechaza() {
    let s = setup();
    let (author, journal, id) = submitted(&s);
    let stranger = Address::generate(&s.env);

    for who in [&stranger, &author] {
        assert_eq!(
            s.splitter.try_accept_manuscript(who, &id),
            Err(Ok(Error::NoApc.into()))
        );
        // Quien envió no puede "rechazarse" para sacar su dinero antes del plazo.
        assert_eq!(
            s.splitter.try_reject_manuscript(who, &id),
            Err(Ok(Error::NoApc.into()))
        );
    }
    assert_eq!(
        s.splitter.try_accept_manuscript(&journal, &99),
        Err(Ok(Error::NoApc.into()))
    );
    assert_eq!(
        s.splitter.try_reject_manuscript(&journal, &99),
        Err(Ok(Error::NoApc.into()))
    );
    s.env.set_auths(&[]);
    assert!(s.splitter.try_accept_manuscript(&journal, &id).is_err());
    assert!(s.splitter.try_reject_manuscript(&journal, &id).is_err());
    assert_eq!(s.usdc.balance(&s.splitter.address), CHARGE);
    assert!(s.splitter.apc(&id).unwrap().open);
}

#[test]
fn la_revista_solo_cobra_si_acepta_antes_del_plazo() {
    let s = setup();
    let (_, journal, on_time) = submitted(&s);
    let (_, late_journal, late) = submitted(&s);

    s.advance(30 * DAY - 1);
    s.splitter.accept_manuscript(&journal, &on_time);
    assert_eq!(s.usdc.balance(&journal), 19_600_000);

    s.advance(1);
    assert_eq!(
        s.splitter.try_accept_manuscript(&late_journal, &late),
        Err(Ok(Error::ApcExpired.into()))
    );
    assert_eq!(s.usdc.balance(&late_journal), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), CHARGE);
}

#[test]
fn una_revista_sin_trustline_no_pierde_el_cobro_queda_pendiente() {
    let s = setup();
    let author = Address::generate(&s.env);
    s.fund(&author, CHARGE);
    let nueva = g_account(&s.env, 71);
    let id = s.splitter.pay_apc(
        &author,
        &nueva,
        &manuscript(&s.env),
        &CHARGE,
        &(s.now() + DAY),
    );
    s.splitter.accept_manuscript(&nueva, &id);

    assert_eq!(s.splitter.owed(&nueva), 19_600_000);
    assert_eq!(s.usdc.balance(&s.treasury), 400_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 19_600_000);
    assert_eq!(s.splitter.claim(&nueva), 19_600_000);
    assert_eq!(s.usdc.balance(&nueva), 19_600_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

// ---------- reject_manuscript ----------

#[test]
fn si_la_revista_rechaza_el_cargo_vuelve_completo_sin_esperar_al_plazo() {
    let s = setup();
    let (author, journal, id) = submitted(&s);
    let back = s.splitter.reject_manuscript(&journal, &id);
    let receipts = s.receipts();

    assert_eq!(back, CHARGE);
    assert_eq!(s.usdc.balance(&author), CHARGE, "completo, sin comisión");
    assert_eq!(s.usdc.balance(&journal), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert!(!s.splitter.apc(&id).unwrap().open);
    let expected = ManuscriptRejected {
        journal: journal.clone(),
        author: author.clone(),
        apc_id: id,
        manuscript_id: manuscript(&s.env),
        amount: CHARGE,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn rechazar_solo_pide_la_firma_de_la_revista_y_tambien_vale_despues_del_plazo() {
    let s = setup();
    let (author, journal, id) = submitted(&s);
    s.advance(40 * DAY);
    s.splitter.reject_manuscript(&journal, &id);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            journal.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "reject_manuscript"),
                    (journal.clone(), id).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
    assert_eq!(s.usdc.balance(&author), CHARGE);
}

#[test]
fn si_quien_envio_no_puede_recibir_la_devolucion_queda_pendiente_a_su_nombre() {
    let s = setup_with(true);
    let (author, journal, id) = submitted(&s);
    let issuer = StellarAssetClient::new(&s.env, &s.usdc.address);
    issuer.set_authorized(&author, &false);

    assert_eq!(s.splitter.reject_manuscript(&journal, &id), CHARGE);
    assert_eq!(s.splitter.owed(&author), CHARGE);
    assert_eq!(s.usdc.balance(&s.splitter.address), CHARGE);

    issuer.set_authorized(&author, &true);
    assert_eq!(s.splitter.claim(&author), CHARGE);
    assert_eq!(s.usdc.balance(&author), CHARGE);
}

// ---------- reclaim_apc ----------

#[test]
fn si_la_revista_no_decide_a_tiempo_quien_envio_recupera_el_cargo() {
    let s = setup();
    let (author, journal, id) = submitted(&s);
    s.advance(30 * DAY);
    let back = s.splitter.reclaim_apc(&author, &id);
    let receipts = s.receipts();

    assert_eq!(back, CHARGE);
    assert_eq!(s.usdc.balance(&author), CHARGE);
    assert_eq!(s.usdc.balance(&journal), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert!(!s.splitter.apc(&id).unwrap().open);
    let expected = ApcReclaimed {
        author: author.clone(),
        apc_id: id,
        amount: CHARGE,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn no_se_recupera_antes_del_plazo_y_solo_lo_hace_quien_envio_con_su_firma() {
    let s = setup();
    let (author, journal, id) = submitted(&s);
    let stranger = Address::generate(&s.env);

    assert_eq!(
        s.splitter.try_reclaim_apc(&author, &id),
        Err(Ok(Error::ApcActive.into()))
    );
    s.advance(30 * DAY - 1);
    assert_eq!(
        s.splitter.try_reclaim_apc(&author, &id),
        Err(Ok(Error::ApcActive.into()))
    );
    s.advance(1);
    for who in [&stranger, &journal] {
        assert_eq!(
            s.splitter.try_reclaim_apc(who, &id),
            Err(Ok(Error::NoApc.into()))
        );
    }
    assert_eq!(
        s.splitter.try_reclaim_apc(&author, &99),
        Err(Ok(Error::NoApc.into()))
    );
    s.env.set_auths(&[]);
    assert!(s.splitter.try_reclaim_apc(&author, &id).is_err());
    assert_eq!(s.usdc.balance(&s.splitter.address), CHARGE);

    s.env.mock_all_auths();
    s.splitter.reclaim_apc(&author, &id);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            author.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "reclaim_apc"),
                    (author.clone(), id).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
    assert_eq!(s.usdc.balance(&author), CHARGE);
}

#[test]
fn un_cargo_tiene_una_sola_salida() {
    let s = setup();
    let (author_a, journal_a, accepted) = submitted(&s);
    let (author_b, journal_b, rejected) = submitted(&s);
    let (author_c, journal_c, reclaimed) = submitted(&s);
    s.splitter.accept_manuscript(&journal_a, &accepted);
    s.splitter.reject_manuscript(&journal_b, &rejected);
    s.advance(30 * DAY);
    s.splitter.reclaim_apc(&author_c, &reclaimed);

    for (author, journal, id) in [
        (&author_a, &journal_a, accepted),
        (&author_b, &journal_b, rejected),
        (&author_c, &journal_c, reclaimed),
    ] {
        assert_eq!(
            s.splitter.try_accept_manuscript(journal, &id),
            Err(Ok(Error::ApcClosed.into()))
        );
        assert_eq!(
            s.splitter.try_reject_manuscript(journal, &id),
            Err(Ok(Error::ApcClosed.into()))
        );
        assert_eq!(
            s.splitter.try_reclaim_apc(author, &id),
            Err(Ok(Error::ApcClosed.into()))
        );
    }
    assert_eq!(s.usdc.balance(&journal_a), 19_600_000);
    assert_eq!(s.usdc.balance(&author_b), CHARGE);
    assert_eq!(s.usdc.balance(&author_c), CHARGE);
    assert_eq!(s.usdc.balance(&s.treasury), 400_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

// ---------- TTL y custodia ----------

#[test]
fn el_cargo_vive_en_la_red_hasta_su_plazo_y_la_gracia_para_recuperarlo() {
    let s = setup();
    let (_, _, id) = submitted(&s);
    let ttl = s.env.as_contract(&s.splitter.address, || {
        s.env.storage().persistent().get_ttl(&DataKey::Apc(id))
    });
    assert_eq!(
        ttl,
        ((30 + REFUND_GRACE_DAYS) as u64 * DAY / 5) as u32 + 7 * DAY_LEDGERS
    );
}

#[test]
fn el_saldo_del_contrato_cuenta_los_cargos_sin_decidir_y_lo_pendiente() {
    let s = setup();
    let nueva = g_account(&s.env, 73);
    let author_d = Address::generate(&s.env);
    s.fund(&author_d, CHARGE);
    let (_, journal_a, to_accept) = submitted(&s);
    let (_, journal_b, to_reject) = submitted(&s);
    let (author_c, _, to_reclaim) = submitted(&s);
    let (_, _, stays) = submitted(&s);
    let to_pending = s.splitter.pay_apc(
        &author_d,
        &nueva,
        &manuscript(&s.env),
        &CHARGE,
        &(s.now() + 30 * DAY),
    );
    let check = || {
        let mut held = s.splitter.owed(&nueva);
        for id in [to_accept, to_reject, to_reclaim, stays, to_pending] {
            let apc = s.splitter.apc(&id).unwrap();
            if apc.open {
                held += apc.amount;
            }
        }
        assert_eq!(s.usdc.balance(&s.splitter.address), held);
    };
    check();

    s.splitter.accept_manuscript(&journal_a, &to_accept);
    check();
    s.splitter.reject_manuscript(&journal_b, &to_reject);
    check();
    s.splitter.accept_manuscript(&nueva, &to_pending);
    check();
    assert_eq!(s.splitter.owed(&nueva), 19_600_000);
    s.advance(30 * DAY);
    s.splitter.reclaim_apc(&author_c, &to_reclaim);
    check();
    s.splitter.claim(&nueva);
    check();
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        CHARGE,
        "queda el cargo sin decidir"
    );
}

// ---------- accept_with_waiver ----------

#[test]
fn aceptar_con_exencion_cobra_una_parte_y_devuelve_la_otra_a_quien_envio() {
    let s = setup();
    let (author, journal, id) = submitted(&s);
    let split = s.splitter.accept_with_waiver(&journal, &id, &5_000_000);
    let receipts = s.receipts();
    let auths = s.env.auths();

    // De 2.00, la revista perdona 0.50 y cobra 1.50: 1.47 para ella y 0.03 de comisión.
    assert_eq!(
        split,
        Split {
            publisher: 14_700_000,
            fee: 300_000
        }
    );
    assert_eq!(s.usdc.balance(&journal), 14_700_000);
    assert_eq!(s.usdc.balance(&s.treasury), 300_000);
    assert_eq!(
        s.usdc.balance(&author),
        5_000_000,
        "lo perdonado vuelve completo, sin comisión"
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert!(!s.splitter.apc(&id).unwrap().open);
    let expected = ManuscriptAccepted {
        journal: journal.clone(),
        author: author.clone(),
        apc_id: id,
        manuscript_id: manuscript(&s.env),
        amount: 15_000_000,
        fee: 300_000,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
    assert_eq!(
        auths,
        std::vec![(
            journal.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "accept_with_waiver"),
                    (journal.clone(), id, 5_000_000i128).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
}

#[test]
fn la_exencion_es_una_parte_del_cargo_y_solo_la_da_la_revista_antes_del_plazo() {
    let s = setup();
    let (author, journal, id) = submitted(&s);
    let (_, late_journal, late) = submitted(&s);
    let waive = |who: &Address, apc: u32, waived: i128| {
        s.splitter.try_accept_with_waiver(who, &apc, &waived)
    };

    // Perdonar nada es `accept_manuscript`; perdonar todo es `reject_manuscript`.
    for waived in [0, -1, CHARGE, CHARGE + 1] {
        assert_eq!(
            waive(&journal, id, waived),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    assert_eq!(waive(&author, id, 5_000_000), Err(Ok(Error::NoApc.into())));
    assert_eq!(waive(&journal, 99, 5_000_000), Err(Ok(Error::NoApc.into())));
    s.env.set_auths(&[]);
    assert!(waive(&journal, id, 5_000_000).is_err());
    s.env.mock_all_auths();
    assert_eq!(s.usdc.balance(&s.splitter.address), 2 * CHARGE);

    // El borde: perdonar todo menos un stroop.
    assert!(waive(&journal, id, CHARGE - 1).is_ok());
    assert_eq!(s.usdc.balance(&author), CHARGE - 1);
    assert_eq!(s.usdc.balance(&journal), 1);
    assert_eq!(waive(&journal, id, 1), Err(Ok(Error::ApcClosed.into())));

    s.advance(30 * DAY);
    assert_eq!(
        waive(&late_journal, late, 5_000_000),
        Err(Ok(Error::ApcExpired.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), CHARGE);
}

#[test]
fn si_quien_envio_no_puede_recibir_lo_perdonado_queda_pendiente_y_la_revista_cobra() {
    let s = setup_with(true);
    let (author, journal, id) = submitted(&s);
    let issuer = StellarAssetClient::new(&s.env, &s.usdc.address);
    issuer.set_authorized(&author, &false);

    s.splitter.accept_with_waiver(&journal, &id, &5_000_000);
    assert_eq!(s.usdc.balance(&journal), 14_700_000);
    assert_eq!(s.splitter.owed(&author), 5_000_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 5_000_000);

    issuer.set_authorized(&author, &true);
    assert_eq!(s.splitter.claim(&author), 5_000_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}
