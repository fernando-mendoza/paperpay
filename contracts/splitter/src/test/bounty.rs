//! Recompensas por revisar o reproducir (v0.8.0): `post_bounty`, `award_bounty` y `cancel_bounty`.

use super::*;
use crate::{Bounty, BountyAwarded, BountyCancelled, BountyPosted, Rewarded, MAX_BOUNTY_DAYS};

/// La recompensa de prueba: 1 USDC.
const REWARD: i128 = 10_000_000;

impl Setup<'_> {
    /// Una cuenta con `amount` de USDC.
    fn funder(&self, amount: i128) -> Address {
        let funder = Address::generate(&self.env);
        self.fund(&funder, amount);
        funder
    }

    /// Una recompensa de 1 USDC con plazo de 7 días. Devuelve quién la publicó y su id.
    fn posted_bounty(&self) -> (Address, u32) {
        let funder = self.funder(REWARD);
        let id =
            self.splitter
                .post_bounty(&funder, &paper(&self.env), &REWARD, &(self.now() + 7 * DAY));
        (funder, id)
    }
}

// ---------- post_bounty ----------

#[test]
fn una_recompensa_queda_en_garantia_hasta_su_plazo() {
    let s = setup();
    let funder = s.funder(2 * REWARD);
    let deadline = s.now() + 7 * DAY;
    let id = s
        .splitter
        .post_bounty(&funder, &paper(&s.env), &REWARD, &deadline);
    let receipts = s.receipts();

    assert_eq!(id, 0);
    assert_eq!(s.usdc.balance(&funder), REWARD);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        REWARD,
        "la recompensa queda en el contrato"
    );
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(
        s.splitter.bounty(&id),
        Some(Bounty {
            funder: funder.clone(),
            paper_id: paper(&s.env),
            amount: REWARD,
            deadline,
            open: true,
        })
    );
    let expected = BountyPosted {
        funder: funder.clone(),
        bounty_id: 0,
        paper_id: paper(&s.env),
        amount: REWARD,
        deadline,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    // Los ids son secuenciales y cada recompensa guarda lo suyo.
    let second = s
        .splitter
        .post_bounty(&funder, &paper(&s.env), &4_000_000, &(deadline + DAY));
    assert_eq!(second, 1);
    assert_eq!(s.splitter.bounty(&second).unwrap().amount, 4_000_000);
    assert_eq!(s.splitter.bounty(&id).unwrap().amount, REWARD);
    assert_eq!(s.splitter.bounty(&2), None);
    assert_eq!(s.usdc.balance(&s.splitter.address), REWARD + 4_000_000);
}

#[test]
fn publicar_solo_pide_la_firma_de_quien_publica() {
    let s = setup();
    let funder = s.funder(REWARD);
    let deadline = s.now() + 7 * DAY;
    s.splitter
        .post_bounty(&funder, &paper(&s.env), &REWARD, &deadline);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            funder.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "post_bounty"),
                    (funder.clone(), paper(&s.env), REWARD, deadline).into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(&funder, &s.splitter.address, REWARD)],
            }
        )]
    );
}

#[test]
fn publicar_rechaza_monto_plazo_e_id_invalidos_y_sin_firma_no_hay_recompensa() {
    let s = setup();
    let funder = s.funder(3 * REWARD);
    let now = s.now();
    let max = MAX_BOUNTY_DAYS as u64 * DAY;

    for amount in [0, -1] {
        assert_eq!(
            s.splitter
                .try_post_bounty(&funder, &paper(&s.env), &amount, &(now + DAY)),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    for deadline in [now, now - 1, now + max + 1] {
        assert_eq!(
            s.splitter
                .try_post_bounty(&funder, &paper(&s.env), &REWARD, &deadline),
            Err(Ok(Error::InvalidDeadline.into()))
        );
    }
    assert_eq!(
        s.splitter.try_post_bounty(
            &funder,
            &String::from_str(&s.env, ""),
            &REWARD,
            &(now + DAY)
        ),
        Err(Ok(Error::InvalidPaperId.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.splitter.bounty(&0), None);

    // Los bordes del plazo sí valen: un segundo después de ahora y justo el máximo.
    assert!(s
        .splitter
        .try_post_bounty(&funder, &paper(&s.env), &REWARD, &(now + 1))
        .is_ok());
    assert!(s
        .splitter
        .try_post_bounty(&funder, &paper(&s.env), &REWARD, &(now + max))
        .is_ok());

    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_post_bounty(&funder, &paper(&s.env), &REWARD, &(now + DAY))
        .is_err());
    assert_eq!(s.usdc.balance(&funder), REWARD);
    assert_eq!(s.splitter.bounty(&2), None);
}

// ---------- award_bounty ----------

#[test]
fn quien_la_publico_la_reparte_entre_los_ganadores_y_la_tesoreria_cobra_el_2() {
    let s = setup();
    let (funder, id) = s.posted_bounty();
    let lab_a = Address::generate(&s.env);
    let lab_b = Address::generate(&s.env);
    let winners = vec![&s.env, share(&lab_a, 6_000), share(&lab_b, 4_000)];
    let split = s.splitter.award_bounty(&funder, &id, &winners);
    let receipts = s.receipts();

    assert_eq!(
        split,
        Split {
            publisher: 9_800_000,
            fee: 200_000
        }
    );
    assert_eq!(s.usdc.balance(&lab_a), 5_880_000);
    assert_eq!(s.usdc.balance(&lab_b), 3_920_000);
    assert_eq!(s.usdc.balance(&s.treasury), 200_000);
    assert_eq!(s.usdc.balance(&funder), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert!(!s.splitter.bounty(&id).unwrap().open);
    assert_eq!(
        receipts,
        [
            Rewarded {
                winner: lab_a.clone(),
                bounty_id: id,
                amount: 5_880_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            Rewarded {
                winner: lab_b.clone(),
                bounty_id: id,
                amount: 3_920_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            BountyAwarded {
                funder: funder.clone(),
                bounty_id: id,
                paper_id: paper(&s.env),
                amount: REWARD,
                fee: 200_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );
}

#[test]
fn el_redondeo_del_reparto_va_al_primer_ganador_y_no_se_pierde_nada() {
    let s = setup();
    let funder = s.funder(1_000_001);
    let id = s
        .splitter
        .post_bounty(&funder, &paper(&s.env), &1_000_001, &(s.now() + DAY));
    let a = Address::generate(&s.env);
    let b = Address::generate(&s.env);
    let c = Address::generate(&s.env);
    let winners = vec![&s.env, share(&a, 3_334), share(&b, 3_333), share(&c, 3_333)];
    let split = s.splitter.award_bounty(&funder, &id, &winners);

    // Comisión: piso de 1 000 001 × 2% = 20 000. A repartir: 980 001.
    assert_eq!(split.fee, 20_000);
    assert_eq!(split.publisher, 980_001);
    assert_eq!(s.usdc.balance(&b), 326_634);
    assert_eq!(s.usdc.balance(&c), 326_634);
    assert_eq!(
        s.usdc.balance(&a),
        326_732 + 1,
        "su piso más el stroop que sobra"
    );
    assert_eq!(s.usdc.balance(&s.treasury), 20_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

#[test]
fn repartir_solo_pide_la_firma_de_quien_la_publico() {
    let s = setup();
    let (funder, id) = s.posted_bounty();
    let winner = Address::generate(&s.env);
    let winners = vec![&s.env, share(&winner, 10_000)];
    s.splitter.award_bounty(&funder, &id, &winners);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            funder.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "award_bounty"),
                    (funder.clone(), id, winners.clone()).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
    assert_eq!(s.usdc.balance(&winner), 9_800_000);
}

#[test]
fn un_ganador_sin_trustline_no_bloquea_a_los_demas_y_su_parte_queda_pendiente() {
    let s = setup();
    let (funder, id) = s.posted_bounty();
    let nuevo = g_account(&s.env, 31);
    let lab = Address::generate(&s.env);
    let winners = vec![&s.env, share(&nuevo, 5_000), share(&lab, 5_000)];
    s.splitter.award_bounty(&funder, &id, &winners);
    let receipts = s.receipts();

    assert_eq!(s.usdc.balance(&lab), 4_900_000);
    assert_eq!(s.splitter.owed(&nuevo), 4_900_000);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        4_900_000,
        "lo pendiente sigue en el contrato"
    );
    assert_eq!(s.usdc.balance(&s.treasury), 200_000);
    assert_eq!(
        receipts,
        [
            Credited {
                account: nuevo.clone(),
                amount: 4_900_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            Rewarded {
                winner: nuevo.clone(),
                bounty_id: id,
                amount: 4_900_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            Rewarded {
                winner: lab.clone(),
                bounty_id: id,
                amount: 4_900_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            BountyAwarded {
                funder: funder.clone(),
                bounty_id: id,
                paper_id: paper(&s.env),
                amount: REWARD,
                fee: 200_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );

    s.splitter.claim(&nuevo);
    assert_eq!(s.usdc.balance(&nuevo), 4_900_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

#[test]
fn no_se_reparte_dos_veces_ni_una_recompensa_ajena_o_inexistente() {
    let s = setup();
    let (funder, id) = s.posted_bounty();
    let winner = Address::generate(&s.env);
    let stranger = Address::generate(&s.env);
    let winners = vec![&s.env, share(&winner, 10_000)];

    assert_eq!(
        s.splitter.try_award_bounty(&stranger, &id, &winners),
        Err(Ok(Error::NoBounty.into()))
    );
    // Ni siquiera quien va a ganar puede adjudicársela.
    assert_eq!(
        s.splitter.try_award_bounty(&winner, &id, &winners),
        Err(Ok(Error::NoBounty.into()))
    );
    assert_eq!(
        s.splitter.try_award_bounty(&funder, &99, &winners),
        Err(Ok(Error::NoBounty.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), REWARD);

    s.env.set_auths(&[]);
    assert!(s.splitter.try_award_bounty(&funder, &id, &winners).is_err());
    assert_eq!(s.usdc.balance(&winner), 0);
    assert!(s.splitter.bounty(&id).unwrap().open);

    s.env.mock_all_auths();
    s.splitter.award_bounty(&funder, &id, &winners);
    assert_eq!(
        s.splitter.try_award_bounty(&funder, &id, &winners),
        Err(Ok(Error::BountyClosed.into()))
    );
    assert_eq!(s.usdc.balance(&winner), 9_800_000);
}

#[test]
fn las_partes_invalidas_y_quien_publico_como_ganador_se_rechazan_sin_mover_nada() {
    let s = setup();
    let (funder, id) = s.posted_bounty();
    let a = Address::generate(&s.env);
    let b = Address::generate(&s.env);

    let invalid = [
        Vec::new(&s.env),
        vec![&s.env, share(&a, 6_000), share(&b, 3_999)],
        vec![&s.env, share(&a, 10_000), share(&b, 0)],
        vec![&s.env, share(&a, 5_000), share(&a, 5_000)],
        // Quien publicó no puede premiarse: sería retirar la recompensa antes del plazo.
        vec![&s.env, share(&funder, 10_000)],
        vec![&s.env, share(&a, 5_000), share(&funder, 5_000)],
    ];
    for winners in invalid {
        assert_eq!(
            s.splitter.try_award_bounty(&funder, &id, &winners),
            Err(Ok(Error::InvalidShares.into()))
        );
    }
    let mut many = Vec::new(&s.env);
    for _ in 0..(MAX_AUTHORS + 1) {
        many.push_back(share(&Address::generate(&s.env), 1));
    }
    assert_eq!(
        s.splitter.try_award_bounty(&funder, &id, &many),
        Err(Ok(Error::TooManyAuthors.into()))
    );

    assert!(s.splitter.bounty(&id).unwrap().open);
    assert_eq!(s.usdc.balance(&s.splitter.address), REWARD);
    assert_eq!(s.usdc.balance(&funder), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
}

// ---------- cancel_bounty ----------

#[test]
fn no_se_cancela_antes_del_plazo_y_despues_vuelve_todo_sin_comision() {
    let s = setup();
    let (funder, id) = s.posted_bounty();

    assert_eq!(
        s.splitter.try_cancel_bounty(&funder, &id),
        Err(Ok(Error::BountyActive.into()))
    );
    // Un segundo antes del plazo todavía no; en el plazo, sí.
    s.advance(7 * DAY - 5);
    s.env.ledger().with_mut(|li| li.timestamp += 4);
    assert_eq!(
        s.splitter.try_cancel_bounty(&funder, &id),
        Err(Ok(Error::BountyActive.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), REWARD);

    s.env.ledger().with_mut(|li| li.timestamp += 1);
    let back = s.splitter.cancel_bounty(&funder, &id);
    let receipts = s.receipts();
    assert_eq!(back, REWARD);
    assert_eq!(s.usdc.balance(&funder), REWARD);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0, "cancelar no paga comisión");
    assert!(!s.splitter.bounty(&id).unwrap().open);
    let expected = BountyCancelled {
        funder: funder.clone(),
        bounty_id: id,
        amount: REWARD,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn cancelar_solo_pide_la_firma_de_quien_la_publico_y_nadie_mas_cancela() {
    let s = setup();
    let (funder, id) = s.posted_bounty();
    let stranger = Address::generate(&s.env);
    s.advance(7 * DAY);

    assert_eq!(
        s.splitter.try_cancel_bounty(&stranger, &id),
        Err(Ok(Error::NoBounty.into()))
    );
    assert_eq!(
        s.splitter.try_cancel_bounty(&funder, &99),
        Err(Ok(Error::NoBounty.into()))
    );
    s.env.set_auths(&[]);
    assert!(s.splitter.try_cancel_bounty(&funder, &id).is_err());
    assert_eq!(s.usdc.balance(&s.splitter.address), REWARD);

    s.env.mock_all_auths();
    s.splitter.cancel_bounty(&funder, &id);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            funder.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "cancel_bounty"),
                    (funder.clone(), id).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
}

#[test]
fn una_cancelada_no_se_reparte_y_una_repartida_no_se_cancela() {
    let s = setup();
    let (funder, cancelled) = s.posted_bounty();
    let (other_funder, awarded) = s.posted_bounty();
    let winner = Address::generate(&s.env);
    let winners = vec![&s.env, share(&winner, 10_000)];
    s.advance(7 * DAY);

    s.splitter.cancel_bounty(&funder, &cancelled);
    assert_eq!(
        s.splitter.try_award_bounty(&funder, &cancelled, &winners),
        Err(Ok(Error::BountyClosed.into()))
    );
    assert_eq!(
        s.splitter.try_cancel_bounty(&funder, &cancelled),
        Err(Ok(Error::BountyClosed.into()))
    );

    // Después del plazo todavía se puede repartir, mientras no se haya cancelado.
    s.splitter.award_bounty(&other_funder, &awarded, &winners);
    assert_eq!(
        s.splitter.try_cancel_bounty(&other_funder, &awarded),
        Err(Ok(Error::BountyClosed.into()))
    );
    assert_eq!(s.usdc.balance(&funder), REWARD);
    assert_eq!(s.usdc.balance(&other_funder), 0);
    assert_eq!(s.usdc.balance(&winner), 9_800_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

// ---------- TTL y custodia ----------

#[test]
fn la_recompensa_vive_en_la_red_hasta_su_plazo_y_la_gracia_para_cancelar() {
    let s = setup();
    let (_, id) = s.posted_bounty();
    let ttl = s.env.as_contract(&s.splitter.address, || {
        s.env.storage().persistent().get_ttl(&DataKey::Bounty(id))
    });
    assert_eq!(
        ttl,
        ((7 + REFUND_GRACE_DAYS) as u64 * DAY / 5) as u32 + 7 * DAY_LEDGERS
    );
}

#[test]
fn el_saldo_del_contrato_cuenta_las_recompensas_abiertas_los_fondos_y_lo_pendiente() {
    let s = setup();
    let nuevo = g_account(&s.env, 33);
    let lab = Address::generate(&s.env);
    let sponsor = s.funder(6_000_000);
    let operator = Address::generate(&s.env);
    let (funder_a, to_award) = s.posted_bounty();
    let (funder_b, to_cancel) = s.posted_bounty();
    let (_, stays) = s.posted_bounty();
    let check = || {
        let mut held = s.splitter.owed(&nuevo) + s.splitter.owed(&lab);
        for id in [to_award, to_cancel, stays] {
            let bounty = s.splitter.bounty(&id).unwrap();
            if bounty.open {
                held += bounty.amount;
            }
        }
        if let Some(pool) = s.splitter.pool(&sponsor, &s.publisher) {
            held += pool.balance;
        }
        assert_eq!(s.usdc.balance(&s.splitter.address), held);
    };
    check();

    s.splitter
        .fund_pool(&sponsor, &s.publisher, &operator, &PRICE, &6_000_000);
    check();
    let winners = vec![&s.env, share(&nuevo, 2_500), share(&lab, 7_500)];
    s.splitter.award_bounty(&funder_a, &to_award, &winners);
    check();
    assert_eq!(s.splitter.owed(&nuevo), 2_450_000);

    s.splitter.pay_sponsored(
        &operator,
        &sponsor,
        &lab,
        &paper(&s.env),
        &s.publisher,
        &PRICE,
    );
    s.advance(7 * DAY);
    s.splitter.cancel_bounty(&funder_b, &to_cancel);
    check();
    s.splitter.claim(&nuevo);
    check();
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        REWARD + 1_000_000,
        "queda la recompensa abierta y el resto del fondo"
    );
}
