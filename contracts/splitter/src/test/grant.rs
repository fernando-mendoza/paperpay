//! Traducción por encargo, pagada por hitos (v0.15.0): `fund_grant`, `release_milestone` y
//! `cancel_grant`.

use super::*;
use crate::{
    Grant, GrantCancelled, GrantFunded, MilestoneReleased, MAX_GRANT_DAYS, MAX_MILESTONES,
};

/// Los tres hitos del encargo de prueba: borrador, revisión y versión final. Suman 2 USDC.
const DRAFT: i128 = 5_000_000;
const REVIEW: i128 = 5_000_000;
const FINAL: i128 = 10_000_000;
const TOTAL: i128 = DRAFT + REVIEW + FINAL;

fn milestones(env: &Env) -> Vec<i128> {
    vec![env, DRAFT, REVIEW, FINAL]
}

/// Una cuenta con `amount` de USDC.
fn patron(s: &Setup, amount: i128) -> Address {
    let patron = Address::generate(&s.env);
    s.fund(&patron, amount);
    patron
}

/// Un encargo de 2 USDC en tres hitos, con plazo de 30 días. Devuelve quién lo paga, quién
/// traduce y su id.
fn funded(s: &Setup) -> (Address, Address, u32) {
    let funder = patron(s, TOTAL);
    let translator = Address::generate(&s.env);
    let id = s.splitter.fund_grant(
        &funder,
        &translator,
        &paper(&s.env),
        &milestones(&s.env),
        &(s.now() + 30 * DAY),
    );
    (funder, translator, id)
}

// ---------- fund_grant ----------

#[test]
fn el_encargo_deja_la_suma_de_los_hitos_en_el_contrato() {
    let s = setup();
    let funder = patron(&s, 2 * TOTAL);
    let translator = Address::generate(&s.env);
    let deadline = s.now() + 30 * DAY;
    let id = s.splitter.fund_grant(
        &funder,
        &translator,
        &paper(&s.env),
        &milestones(&s.env),
        &deadline,
    );
    let receipts = s.receipts();

    assert_eq!(id, 0);
    assert_eq!(s.usdc.balance(&funder), TOTAL);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        TOTAL,
        "todos los hitos quedan en el contrato"
    );
    assert_eq!(s.usdc.balance(&translator), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(
        s.splitter.grant(&id),
        Some(Grant {
            funder: funder.clone(),
            translator: translator.clone(),
            paper_id: paper(&s.env),
            milestones: milestones(&s.env),
            released: 0,
            deadline,
            open: true,
        })
    );
    let expected = GrantFunded {
        funder: funder.clone(),
        translator: translator.clone(),
        grant_id: 0,
        paper_id: paper(&s.env),
        total: TOTAL,
        milestones: 3,
        deadline,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    // Los ids son secuenciales y cada encargo guarda lo suyo.
    let second = s.splitter.fund_grant(
        &funder,
        &translator,
        &paper(&s.env),
        &vec![&s.env, 4_000_000],
        &(deadline + DAY),
    );
    assert_eq!(second, 1);
    assert_eq!(s.splitter.grant(&second).unwrap().milestones.len(), 1);
    assert_eq!(s.splitter.grant(&id).unwrap().milestones.len(), 3);
    assert_eq!(s.splitter.grant(&2), None);
    assert_eq!(s.usdc.balance(&s.splitter.address), TOTAL + 4_000_000);
}

#[test]
fn encargar_solo_pide_la_firma_de_quien_paga() {
    let s = setup();
    let funder = patron(&s, TOTAL);
    let translator = Address::generate(&s.env);
    let deadline = s.now() + 30 * DAY;
    s.splitter.fund_grant(
        &funder,
        &translator,
        &paper(&s.env),
        &milestones(&s.env),
        &deadline,
    );

    assert_eq!(
        s.env.auths(),
        std::vec![(
            funder.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "fund_grant"),
                    (
                        funder.clone(),
                        translator.clone(),
                        paper(&s.env),
                        milestones(&s.env),
                        deadline
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(&funder, &s.splitter.address, TOTAL)],
            }
        )]
    );
}

#[test]
fn encargar_rechaza_hitos_plazo_e_id_invalidos_y_sin_firma_no_hay_encargo() {
    let s = setup();
    let funder = patron(&s, 3 * TOTAL);
    let translator = Address::generate(&s.env);
    let now = s.now();
    let max = MAX_GRANT_DAYS as u64 * DAY;
    let fund = |list: &Vec<i128>, deadline: u64| {
        s.splitter
            .try_fund_grant(&funder, &translator, &paper(&s.env), list, &deadline)
    };

    let mut too_many: Vec<i128> = Vec::new(&s.env);
    for _ in 0..=MAX_MILESTONES {
        too_many.push_back(1);
    }
    for list in [
        Vec::new(&s.env),
        too_many,
        vec![&s.env, DRAFT, 0],
        vec![&s.env, -1, DRAFT],
    ] {
        assert_eq!(
            fund(&list, now + DAY),
            Err(Ok(Error::InvalidMilestones.into()))
        );
    }
    assert_eq!(
        fund(&vec![&s.env, i128::MAX, 1], now + DAY),
        Err(Ok(Error::AmountOverflow.into()))
    );
    for deadline in [now, now - 1, now + max + 1] {
        assert_eq!(
            fund(&milestones(&s.env), deadline),
            Err(Ok(Error::InvalidDeadline.into()))
        );
    }
    assert_eq!(
        s.splitter.try_fund_grant(
            &funder,
            &translator,
            &String::from_str(&s.env, ""),
            &milestones(&s.env),
            &(now + DAY)
        ),
        Err(Ok(Error::InvalidPaperId.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.splitter.grant(&0), None);

    // Los bordes del plazo sí valen: un segundo después de ahora y justo el máximo.
    assert!(fund(&milestones(&s.env), now + 1).is_ok());
    assert!(fund(&milestones(&s.env), now + max).is_ok());

    s.env.set_auths(&[]);
    assert!(fund(&milestones(&s.env), now + DAY).is_err());
    assert_eq!(s.usdc.balance(&funder), TOTAL);
    assert_eq!(s.splitter.grant(&2), None);
}

#[test]
fn un_encargo_acepta_el_maximo_de_hitos() {
    let s = setup();
    let funder = patron(&s, 10 * PRICE);
    let translator = Address::generate(&s.env);
    let mut list: Vec<i128> = Vec::new(&s.env);
    for _ in 0..MAX_MILESTONES {
        list.push_back(PRICE);
    }
    let id = s.splitter.fund_grant(
        &funder,
        &translator,
        &paper(&s.env),
        &list,
        &(s.now() + DAY),
    );

    for _ in 0..MAX_MILESTONES {
        s.splitter.release_milestone(&funder, &id);
    }
    assert_eq!(s.usdc.balance(&translator), 10 * 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 10 * 100_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert!(!s.splitter.grant(&id).unwrap().open);
}

// ---------- release_milestone ----------

#[test]
fn liberar_un_hito_le_paga_a_quien_traduce_y_el_2_va_a_la_tesoreria() {
    let s = setup();
    let (funder, translator, id) = funded(&s);
    let split = s.splitter.release_milestone(&funder, &id);
    let receipts = s.receipts();

    assert_eq!(
        split,
        Split {
            publisher: 4_900_000,
            fee: 100_000
        }
    );
    assert_eq!(s.usdc.balance(&translator), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        REVIEW + FINAL,
        "los otros hitos siguen en el contrato"
    );
    let grant = s.splitter.grant(&id).unwrap();
    assert_eq!(grant.released, 1);
    assert!(grant.open);
    let expected = MilestoneReleased {
        translator: translator.clone(),
        funder: funder.clone(),
        grant_id: id,
        milestone: 0,
        amount: DRAFT,
        fee: 100_000,
        remaining: 2,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn los_hitos_se_liberan_en_orden_y_el_ultimo_cierra_el_encargo() {
    let s = setup();
    let (funder, translator, id) = funded(&s);

    s.splitter.release_milestone(&funder, &id);
    s.splitter.release_milestone(&funder, &id);
    assert_eq!(s.usdc.balance(&translator), 2 * 4_900_000);
    assert!(s.splitter.grant(&id).unwrap().open);

    let last = s.splitter.release_milestone(&funder, &id);
    let receipts = s.receipts();
    assert_eq!(
        last,
        Split {
            publisher: 9_800_000,
            fee: 200_000
        }
    );
    let expected = MilestoneReleased {
        translator: translator.clone(),
        funder: funder.clone(),
        grant_id: id,
        milestone: 2,
        amount: FINAL,
        fee: 200_000,
        remaining: 0,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
    let grant = s.splitter.grant(&id).unwrap();
    assert_eq!(grant.released, 3);
    assert!(!grant.open);
    assert_eq!(s.usdc.balance(&translator), 19_600_000);
    assert_eq!(s.usdc.balance(&s.treasury), 400_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);

    assert_eq!(
        s.splitter.try_release_milestone(&funder, &id),
        Err(Ok(Error::GrantClosed.into()))
    );
}

#[test]
fn liberar_solo_pide_la_firma_de_quien_paga() {
    let s = setup();
    let (funder, _, id) = funded(&s);
    s.splitter.release_milestone(&funder, &id);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            funder.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "release_milestone"),
                    (funder.clone(), id).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
}

#[test]
fn solo_quien_paga_libera_ni_quien_traduce_ni_un_extrano() {
    let s = setup();
    let (funder, translator, id) = funded(&s);
    let stranger = Address::generate(&s.env);

    assert_eq!(
        s.splitter.try_release_milestone(&stranger, &id),
        Err(Ok(Error::NoGrant.into()))
    );
    // Quien traduce no puede pagarse solo.
    assert_eq!(
        s.splitter.try_release_milestone(&translator, &id),
        Err(Ok(Error::NoGrant.into()))
    );
    assert_eq!(
        s.splitter.try_release_milestone(&funder, &99),
        Err(Ok(Error::NoGrant.into()))
    );
    s.env.set_auths(&[]);
    assert!(s.splitter.try_release_milestone(&funder, &id).is_err());
    assert_eq!(s.usdc.balance(&s.splitter.address), TOTAL);
    assert_eq!(s.usdc.balance(&translator), 0);
    assert_eq!(s.splitter.grant(&id).unwrap().released, 0);
}

#[test]
fn la_comision_de_cada_hito_se_redondea_a_favor_de_quien_traduce() {
    let s = setup();
    let funder = patron(&s, 1_000_049 + 49);
    let translator = Address::generate(&s.env);
    let id = s.splitter.fund_grant(
        &funder,
        &translator,
        &paper(&s.env),
        &vec![&s.env, 1_000_049, 49],
        &(s.now() + DAY),
    );

    // Piso de 1 000 049 × 2% = 20 000 (sobran 0.98 stroops, que quedan para quien traduce).
    let first = s.splitter.release_milestone(&funder, &id);
    assert_eq!(first.fee, 20_000);
    assert_eq!(first.publisher, 980_049);
    // Un hito de 49 stroops no alcanza para un stroop de comisión.
    let second = s.splitter.release_milestone(&funder, &id);
    assert_eq!(second.fee, 0);
    assert_eq!(s.usdc.balance(&translator), 980_049 + 49);
    assert_eq!(s.usdc.balance(&s.treasury), 20_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

#[test]
fn si_quien_traduce_no_tiene_trustline_su_hito_queda_pendiente_y_lo_cobra_despues() {
    let s = setup();
    let funder = patron(&s, TOTAL);
    let nueva = g_account(&s.env, 51);
    let id = s.splitter.fund_grant(
        &funder,
        &nueva,
        &paper(&s.env),
        &milestones(&s.env),
        &(s.now() + 30 * DAY),
    );
    s.splitter.release_milestone(&funder, &id);
    let receipts = s.receipts();

    assert_eq!(s.splitter.owed(&nueva), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        TOTAL - 100_000,
        "lo pendiente y los otros hitos siguen en el contrato"
    );
    assert_eq!(
        receipts,
        [
            Credited {
                account: nueva.clone(),
                amount: 4_900_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            MilestoneReleased {
                translator: nueva.clone(),
                funder: funder.clone(),
                grant_id: id,
                milestone: 0,
                amount: DRAFT,
                fee: 100_000,
                remaining: 2,
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );

    s.splitter.claim(&nueva);
    assert_eq!(s.usdc.balance(&nueva), 4_900_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), REVIEW + FINAL);
}

// ---------- cancel_grant ----------

#[test]
fn pasado_el_plazo_quien_paga_recupera_los_hitos_no_liberados_sin_comision() {
    let s = setup();
    let (funder, translator, id) = funded(&s);
    s.splitter.release_milestone(&funder, &id);
    s.advance(30 * DAY);
    let back = s.splitter.cancel_grant(&funder, &id);
    let receipts = s.receipts();

    assert_eq!(back, REVIEW + FINAL);
    assert_eq!(s.usdc.balance(&funder), REVIEW + FINAL);
    assert_eq!(
        s.usdc.balance(&translator),
        4_900_000,
        "lo ya pagado no vuelve"
    );
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    let grant = s.splitter.grant(&id).unwrap();
    assert!(!grant.open);
    assert_eq!(grant.released, 1);
    let expected = GrantCancelled {
        funder: funder.clone(),
        grant_id: id,
        amount: REVIEW + FINAL,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn no_se_cancela_antes_del_plazo_y_si_en_el_segundo_exacto() {
    let s = setup();
    let (funder, _, id) = funded(&s);

    assert_eq!(
        s.splitter.try_cancel_grant(&funder, &id),
        Err(Ok(Error::GrantActive.into()))
    );
    s.advance(30 * DAY - 1);
    assert_eq!(
        s.splitter.try_cancel_grant(&funder, &id),
        Err(Ok(Error::GrantActive.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), TOTAL);

    s.advance(1);
    assert_eq!(s.splitter.cancel_grant(&funder, &id), TOTAL);
    assert_eq!(s.usdc.balance(&funder), TOTAL);
}

#[test]
fn solo_quien_paga_cancela_y_solo_con_su_firma() {
    let s = setup();
    let (funder, translator, id) = funded(&s);
    let stranger = Address::generate(&s.env);
    s.advance(30 * DAY);

    assert_eq!(
        s.splitter.try_cancel_grant(&stranger, &id),
        Err(Ok(Error::NoGrant.into()))
    );
    assert_eq!(
        s.splitter.try_cancel_grant(&translator, &id),
        Err(Ok(Error::NoGrant.into()))
    );
    assert_eq!(
        s.splitter.try_cancel_grant(&funder, &99),
        Err(Ok(Error::NoGrant.into()))
    );
    s.env.set_auths(&[]);
    assert!(s.splitter.try_cancel_grant(&funder, &id).is_err());
    assert_eq!(s.usdc.balance(&s.splitter.address), TOTAL);

    s.env.mock_all_auths();
    s.splitter.cancel_grant(&funder, &id);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            funder.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "cancel_grant"),
                    (funder.clone(), id).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
}

#[test]
fn un_encargo_cancelado_no_libera_y_uno_terminado_no_se_cancela() {
    let s = setup();
    let (funder, translator, cancelled) = funded(&s);
    let (other_funder, other_translator, finished) = funded(&s);
    s.advance(30 * DAY);

    // Después del plazo todavía se puede liberar, mientras no se haya cancelado.
    for _ in 0..3 {
        s.splitter.release_milestone(&other_funder, &finished);
    }
    assert_eq!(
        s.splitter.try_cancel_grant(&other_funder, &finished),
        Err(Ok(Error::GrantClosed.into()))
    );

    s.splitter.cancel_grant(&funder, &cancelled);
    assert_eq!(
        s.splitter.try_release_milestone(&funder, &cancelled),
        Err(Ok(Error::GrantClosed.into()))
    );
    assert_eq!(
        s.splitter.try_cancel_grant(&funder, &cancelled),
        Err(Ok(Error::GrantClosed.into()))
    );
    assert_eq!(s.usdc.balance(&funder), TOTAL);
    assert_eq!(s.usdc.balance(&translator), 0);
    assert_eq!(s.usdc.balance(&other_translator), 19_600_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

// ---------- TTL y custodia ----------

#[test]
fn el_encargo_vive_en_la_red_hasta_su_plazo_y_la_gracia_para_cancelarlo() {
    let s = setup();
    let (_, _, id) = funded(&s);
    let ttl = s.env.as_contract(&s.splitter.address, || {
        s.env.storage().persistent().get_ttl(&DataKey::Grant(id))
    });
    assert_eq!(
        ttl,
        ((30 + REFUND_GRACE_DAYS) as u64 * DAY / 5) as u32 + 7 * DAY_LEDGERS
    );
}

#[test]
fn el_saldo_del_contrato_cuenta_los_hitos_sin_liberar_y_lo_pendiente() {
    let s = setup();
    let nueva = g_account(&s.env, 53);
    let (funder_a, _, first) = funded(&s);
    let (funder_b, _, second) = funded(&s);
    let funder_c = patron(&s, TOTAL);
    let third = s.splitter.fund_grant(
        &funder_c,
        &nueva,
        &paper(&s.env),
        &milestones(&s.env),
        &(s.now() + 30 * DAY),
    );
    let check = || {
        let mut held = s.splitter.owed(&nueva);
        for id in [first, second, third] {
            let grant = s.splitter.grant(&id).unwrap();
            if grant.open {
                for index in grant.released..grant.milestones.len() {
                    held += grant.milestones.get_unchecked(index);
                }
            }
        }
        assert_eq!(s.usdc.balance(&s.splitter.address), held);
    };
    check();

    s.splitter.release_milestone(&funder_a, &first);
    check();
    s.splitter.release_milestone(&funder_c, &third);
    check();
    assert_eq!(s.splitter.owed(&nueva), 4_900_000);
    for _ in 0..3 {
        s.splitter.release_milestone(&funder_b, &second);
    }
    check();

    s.advance(30 * DAY);
    s.splitter.cancel_grant(&funder_a, &first);
    check();
    s.splitter.claim(&nueva);
    check();
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        REVIEW + FINAL,
        "quedan los dos hitos sin liberar del tercer encargo"
    );
}

// ---------- add_milestone ----------

#[test]
fn agregar_un_hito_deposita_mas_y_se_libera_al_final_como_los_demas() {
    let s = setup();
    let (funder, translator, id) = funded(&s);
    s.fund(&funder, 3_000_000);
    let count = s.splitter.add_milestone(&funder, &id, &3_000_000);
    let receipts = s.receipts();
    let auths = s.env.auths();

    assert_eq!(count, 4);
    assert_eq!(s.usdc.balance(&funder), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), TOTAL + 3_000_000);
    let grant = s.splitter.grant(&id).unwrap();
    assert_eq!(
        grant.milestones,
        vec![&s.env, DRAFT, REVIEW, FINAL, 3_000_000]
    );
    assert_eq!(grant.released, 0);
    let expected = GrantFunded {
        funder: funder.clone(),
        translator: translator.clone(),
        grant_id: id,
        paper_id: paper(&s.env),
        total: 3_000_000,
        milestones: 4,
        deadline: grant.deadline,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
    assert_eq!(
        auths,
        std::vec![(
            funder.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "add_milestone"),
                    (funder.clone(), id, 3_000_000i128).into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(
                    &funder,
                    &s.splitter.address,
                    3_000_000
                )],
            }
        )]
    );

    for _ in 0..4 {
        s.splitter.release_milestone(&funder, &id);
    }
    assert_eq!(s.usdc.balance(&translator), 19_600_000 + 2_940_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert!(!s.splitter.grant(&id).unwrap().open);
}

#[test]
fn solo_quien_paga_agrega_hitos_hasta_el_maximo_y_a_un_encargo_abierto() {
    let s = setup();
    let (funder, translator, id) = funded(&s);
    s.fund(&funder, 10 * PRICE);
    let add_one = |who: &Address, grant: u32, amount: i128| {
        s.splitter.try_add_milestone(who, &grant, &amount)
    };

    for amount in [0, -1] {
        assert_eq!(
            add_one(&funder, id, amount),
            Err(Ok(Error::InvalidMilestones.into()))
        );
    }
    assert_eq!(
        add_one(&translator, id, PRICE),
        Err(Ok(Error::NoGrant.into()))
    );
    assert_eq!(add_one(&funder, 99, PRICE), Err(Ok(Error::NoGrant.into())));
    s.env.set_auths(&[]);
    assert!(add_one(&funder, id, PRICE).is_err());
    s.env.mock_all_auths();
    assert_eq!(s.usdc.balance(&s.splitter.address), TOTAL);

    // De 3 a 10 hitos caben 7 más; el undécimo no.
    for _ in 0..7 {
        s.splitter.add_milestone(&funder, &id, &PRICE);
    }
    assert_eq!(
        add_one(&funder, id, PRICE),
        Err(Ok(Error::InvalidMilestones.into()))
    );

    // Cancelar devuelve también los hitos agregados, y a un encargo cerrado no se le agrega.
    s.advance(30 * DAY);
    assert_eq!(s.splitter.cancel_grant(&funder, &id), TOTAL + 7 * PRICE);
    assert_eq!(
        add_one(&funder, id, PRICE),
        Err(Ok(Error::GrantClosed.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}
