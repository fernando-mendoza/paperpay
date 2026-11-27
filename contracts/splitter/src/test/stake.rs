//! Compromiso del revisor (v0.14.0): `accept_bounty`, `submit_review` y `forfeit_stake`.

use super::*;
use crate::{BountyAccepted, ReviewSubmitted, StakeForfeited};
use soroban_sdk::BytesN;

/// La recompensa de prueba: 1 USDC.
const REWARD: i128 = 10_000_000;
/// La garantía mínima para esa recompensa: el 10%.
const STAKE: i128 = 1_000_000;

/// Una recompensa de 1 USDC con plazo de 7 días. Devuelve quién la publicó y su id.
fn posted(s: &Setup) -> (Address, u32) {
    let funder = Address::generate(&s.env);
    s.fund(&funder, REWARD);
    let id = s
        .splitter
        .post_bounty(&funder, &paper(&s.env), &REWARD, &(s.now() + 7 * DAY));
    (funder, id)
}

/// Una cuenta con USDC para dos garantías.
fn reviewer(s: &Setup) -> Address {
    let reviewer = Address::generate(&s.env);
    s.fund(&reviewer, 2 * STAKE);
    reviewer
}

/// El sha256 del informe entregado.
fn report(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[9; 32])
}

// ---------- accept_bounty ----------

#[test]
fn tomar_una_recompensa_deja_la_garantia_en_el_contrato() {
    let s = setup();
    let (funder, id) = posted(&s);
    let ana = reviewer(&s);
    let deadline = s.splitter.accept_bounty(&ana, &id, &STAKE);
    let receipts = s.receipts();

    assert_eq!(deadline, s.now() + 7 * DAY);
    assert_eq!(s.usdc.balance(&ana), STAKE);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        REWARD + STAKE,
        "la garantía queda en el contrato, junto a la recompensa"
    );
    assert_eq!(s.usdc.balance(&funder), 0);
    assert_eq!(s.splitter.stake(&id, &ana), STAKE);
    let expected = BountyAccepted {
        reviewer: ana.clone(),
        bounty_id: id,
        stake: STAKE,
        deadline,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    // Otra persona puede tomar la misma recompensa, con una garantía mayor: cada una guarda lo suyo.
    let beto = reviewer(&s);
    s.splitter.accept_bounty(&beto, &id, &(STAKE + 500_000));
    assert_eq!(s.splitter.stake(&id, &beto), STAKE + 500_000);
    assert_eq!(s.splitter.stake(&id, &ana), STAKE);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        REWARD + 2 * STAKE + 500_000
    );
    // La recompensa no cambia.
    assert_eq!(s.splitter.bounty(&id).unwrap().amount, REWARD);
    assert!(s.splitter.bounty(&id).unwrap().open);
}

#[test]
fn tomar_solo_pide_la_firma_de_quien_revisa() {
    let s = setup();
    let (_, id) = posted(&s);
    let ana = reviewer(&s);
    s.splitter.accept_bounty(&ana, &id, &STAKE);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            ana.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "accept_bounty"),
                    (ana.clone(), id, STAKE).into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(&ana, &s.splitter.address, STAKE)],
            }
        )]
    );
}

#[test]
fn la_garantia_es_al_menos_el_10_por_ciento_y_una_cuenta_la_toma_una_vez() {
    let s = setup();
    let (_, id) = posted(&s);
    let ana = reviewer(&s);

    for stake in [0, -1] {
        assert_eq!(
            s.splitter.try_accept_bounty(&ana, &id, &stake),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    assert_eq!(
        s.splitter.try_accept_bounty(&ana, &id, &(STAKE - 1)),
        Err(Ok(Error::StakeTooLow.into()))
    );
    assert_eq!(
        s.splitter.try_accept_bounty(&ana, &99, &STAKE),
        Err(Ok(Error::NoBounty.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), REWARD);
    assert_eq!(s.splitter.stake(&id, &ana), 0);

    // Justo el 10% sí vale.
    s.splitter.accept_bounty(&ana, &id, &STAKE);
    assert_eq!(
        s.splitter.try_accept_bounty(&ana, &id, &STAKE),
        Err(Ok(Error::AlreadyAccepted.into()))
    );
    assert_eq!(s.usdc.balance(&ana), STAKE);
    assert_eq!(s.splitter.stake(&id, &ana), STAKE);

    let beto = reviewer(&s);
    s.env.set_auths(&[]);
    assert!(s.splitter.try_accept_bounty(&beto, &id, &STAKE).is_err());
    assert_eq!(s.splitter.stake(&id, &beto), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), REWARD + STAKE);
}

#[test]
fn no_se_toma_una_recompensa_vencida_ni_una_ya_repartida() {
    let s = setup();
    let (_, id) = posted(&s);
    let (funder, awarded) = posted(&s);
    let ana = reviewer(&s);
    let beto = reviewer(&s);
    let winners = vec![&s.env, share(&beto, 10_000)];
    s.splitter.award_bounty(&funder, &awarded, &winners);

    assert_eq!(
        s.splitter.try_accept_bounty(&ana, &awarded, &STAKE),
        Err(Ok(Error::BountyClosed.into()))
    );

    // Un segundo antes del plazo todavía se puede tomar; en el segundo del plazo, ya no.
    s.advance(7 * DAY - 1);
    s.splitter.accept_bounty(&ana, &id, &STAKE);
    s.advance(1);
    assert_eq!(
        s.splitter.try_accept_bounty(&beto, &id, &STAKE),
        Err(Ok(Error::BountyExpired.into()))
    );
    assert_eq!(s.splitter.stake(&id, &beto), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), REWARD + STAKE);
}

// ---------- submit_review ----------

#[test]
fn quien_entrega_a_tiempo_recupera_su_garantia_y_el_hash_del_informe_queda_en_la_red() {
    let s = setup();
    let (funder, id) = posted(&s);
    let ana = reviewer(&s);
    s.splitter.accept_bounty(&ana, &id, &STAKE);
    let back = s.splitter.submit_review(&ana, &id, &report(&s.env));
    let receipts = s.receipts();

    assert_eq!(back, STAKE);
    assert_eq!(s.usdc.balance(&ana), 2 * STAKE);
    assert_eq!(s.splitter.stake(&id, &ana), 0);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        REWARD,
        "la recompensa sigue en garantía"
    );
    assert_eq!(s.usdc.balance(&funder), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert!(s.splitter.bounty(&id).unwrap().open);
    let expected = ReviewSubmitted {
        reviewer: ana.clone(),
        bounty_id: id,
        report_hash: report(&s.env),
        stake: STAKE,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn entregar_solo_pide_la_firma_de_quien_revisa() {
    let s = setup();
    let (_, id) = posted(&s);
    let ana = reviewer(&s);
    s.splitter.accept_bounty(&ana, &id, &STAKE);
    s.splitter.submit_review(&ana, &id, &report(&s.env));

    assert_eq!(
        s.env.auths(),
        std::vec![(
            ana.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "submit_review"),
                    (ana.clone(), id, report(&s.env)).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
}

#[test]
fn solo_entrega_quien_dejo_garantia_una_vez_y_con_su_firma() {
    let s = setup();
    let (funder, id) = posted(&s);
    let ana = reviewer(&s);
    let stranger = reviewer(&s);
    s.splitter.accept_bounty(&ana, &id, &STAKE);

    assert_eq!(
        s.splitter
            .try_submit_review(&stranger, &id, &report(&s.env)),
        Err(Ok(Error::NoStake.into()))
    );
    // Quien publicó la recompensa no puede "entregar" para quedarse con una garantía ajena.
    assert_eq!(
        s.splitter.try_submit_review(&funder, &id, &report(&s.env)),
        Err(Ok(Error::NoStake.into()))
    );
    assert_eq!(
        s.splitter.try_submit_review(&ana, &99, &report(&s.env)),
        Err(Ok(Error::NoStake.into()))
    );
    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_submit_review(&ana, &id, &report(&s.env))
        .is_err());
    assert_eq!(s.splitter.stake(&id, &ana), STAKE);

    s.env.mock_all_auths();
    s.splitter.submit_review(&ana, &id, &report(&s.env));
    assert_eq!(
        s.splitter.try_submit_review(&ana, &id, &report(&s.env)),
        Err(Ok(Error::NoStake.into()))
    );
    assert_eq!(s.usdc.balance(&ana), 2 * STAKE);
    assert_eq!(s.usdc.balance(&s.splitter.address), REWARD);
}

#[test]
fn la_entrega_solo_vale_antes_del_plazo() {
    let s = setup();
    let (_, id) = posted(&s);
    let ana = reviewer(&s);
    let beto = reviewer(&s);
    s.splitter.accept_bounty(&ana, &id, &STAKE);
    s.splitter.accept_bounty(&beto, &id, &STAKE);

    s.advance(7 * DAY - 1);
    assert_eq!(s.splitter.submit_review(&ana, &id, &report(&s.env)), STAKE);

    s.advance(1);
    assert_eq!(
        s.splitter.try_submit_review(&beto, &id, &report(&s.env)),
        Err(Ok(Error::BountyExpired.into()))
    );
    assert_eq!(s.splitter.stake(&id, &beto), STAKE);
    assert_eq!(s.usdc.balance(&beto), STAKE);
}

#[test]
fn si_la_recompensa_se_reparte_antes_quien_la_tomo_aun_puede_entregar_y_recuperar() {
    let s = setup();
    let (funder, id) = posted(&s);
    let ana = reviewer(&s);
    let beto = reviewer(&s);
    s.splitter.accept_bounty(&ana, &id, &STAKE);
    s.splitter.accept_bounty(&beto, &id, &STAKE);

    // Gana Beto antes del plazo. La garantía de Ana no queda atrapada: tiene hasta el plazo.
    let winners = vec![&s.env, share(&beto, 10_000)];
    s.splitter.award_bounty(&funder, &id, &winners);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        2 * STAKE,
        "repartir no toca las garantías"
    );

    assert_eq!(s.splitter.submit_review(&ana, &id, &report(&s.env)), STAKE);
    assert_eq!(s.splitter.submit_review(&beto, &id, &report(&s.env)), STAKE);
    assert_eq!(s.usdc.balance(&ana), 2 * STAKE);
    assert_eq!(s.usdc.balance(&beto), 2 * STAKE + 9_800_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

// ---------- forfeit_stake ----------

#[test]
fn quien_no_entrega_a_tiempo_pierde_la_garantia_a_favor_de_quien_publico() {
    let s = setup();
    let (funder, id) = posted(&s);
    let ana = reviewer(&s);
    s.splitter.accept_bounty(&ana, &id, &STAKE);
    s.advance(7 * DAY);

    // No pide firma: cualquiera la ejecuta y el dinero solo puede ir a quien publicó.
    s.env.set_auths(&[]);
    let lost = s.splitter.forfeit_stake(&id, &ana);
    let receipts = s.receipts();

    assert_eq!(s.env.auths(), std::vec![]);
    assert_eq!(lost, STAKE);
    assert_eq!(s.usdc.balance(&funder), STAKE, "completa, sin comisión");
    assert_eq!(s.usdc.balance(&ana), STAKE);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(s.splitter.stake(&id, &ana), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), REWARD);
    let expected = StakeForfeited {
        reviewer: ana.clone(),
        funder: funder.clone(),
        bounty_id: id,
        amount: STAKE,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn la_garantia_no_se_pierde_antes_del_plazo_y_si_en_el_segundo_exacto() {
    let s = setup();
    let (funder, id) = posted(&s);
    let ana = reviewer(&s);
    s.splitter.accept_bounty(&ana, &id, &STAKE);

    assert_eq!(
        s.splitter.try_forfeit_stake(&id, &ana),
        Err(Ok(Error::BountyActive.into()))
    );
    s.advance(7 * DAY - 1);
    assert_eq!(
        s.splitter.try_forfeit_stake(&id, &ana),
        Err(Ok(Error::BountyActive.into()))
    );
    assert_eq!(s.splitter.stake(&id, &ana), STAKE);
    assert_eq!(s.usdc.balance(&funder), 0);

    s.advance(1);
    assert_eq!(s.splitter.forfeit_stake(&id, &ana), STAKE);
    assert_eq!(s.usdc.balance(&funder), STAKE);
}

#[test]
fn no_se_pierde_una_garantia_entregada_inexistente_ni_dos_veces() {
    let s = setup();
    let (funder, id) = posted(&s);
    let ana = reviewer(&s);
    let beto = reviewer(&s);
    let nadie = reviewer(&s);
    s.splitter.accept_bounty(&ana, &id, &STAKE);
    s.splitter.accept_bounty(&beto, &id, &STAKE);
    s.splitter.submit_review(&ana, &id, &report(&s.env));
    s.advance(7 * DAY);

    assert_eq!(
        s.splitter.try_forfeit_stake(&id, &ana),
        Err(Ok(Error::NoStake.into()))
    );
    assert_eq!(
        s.splitter.try_forfeit_stake(&id, &nadie),
        Err(Ok(Error::NoStake.into()))
    );
    assert_eq!(
        s.splitter.try_forfeit_stake(&99, &beto),
        Err(Ok(Error::NoStake.into()))
    );
    s.splitter.forfeit_stake(&id, &beto);
    assert_eq!(
        s.splitter.try_forfeit_stake(&id, &beto),
        Err(Ok(Error::NoStake.into()))
    );
    assert_eq!(s.usdc.balance(&funder), STAKE);
    assert_eq!(s.usdc.balance(&ana), 2 * STAKE);
    assert_eq!(s.usdc.balance(&beto), STAKE);
    assert_eq!(s.usdc.balance(&s.splitter.address), REWARD);
}

#[test]
fn si_quien_publico_no_puede_recibir_la_garantia_perdida_queda_pendiente_a_su_nombre() {
    let s = setup_with(true);
    let (funder, id) = posted(&s);
    let ana = reviewer(&s);
    s.splitter.accept_bounty(&ana, &id, &STAKE);
    s.advance(7 * DAY);
    let issuer = StellarAssetClient::new(&s.env, &s.usdc.address);
    issuer.set_authorized(&funder, &false);

    assert_eq!(s.splitter.forfeit_stake(&id, &ana), STAKE);
    let receipts = s.receipts();
    assert_eq!(s.splitter.owed(&funder), STAKE);
    assert_eq!(s.splitter.stake(&id, &ana), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), REWARD + STAKE);
    assert_eq!(
        receipts,
        [
            Credited {
                account: funder.clone(),
                amount: STAKE,
            }
            .to_xdr(&s.env, &s.splitter.address),
            StakeForfeited {
                reviewer: ana.clone(),
                funder: funder.clone(),
                bounty_id: id,
                amount: STAKE,
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );

    issuer.set_authorized(&funder, &true);
    assert_eq!(s.splitter.claim(&funder), STAKE);
    assert_eq!(s.usdc.balance(&funder), STAKE);
}

#[test]
fn vencida_la_recompensa_quien_publico_recupera_su_dinero_y_la_garantia_perdida() {
    let s = setup();
    let (funder, id) = posted(&s);
    let ana = reviewer(&s);
    s.splitter.accept_bounty(&ana, &id, &STAKE);
    s.advance(7 * DAY);

    // Cancelar no depende de las garantías ni las toca.
    assert_eq!(s.splitter.cancel_bounty(&funder, &id), REWARD);
    assert_eq!(s.usdc.balance(&s.splitter.address), STAKE);
    assert_eq!(s.splitter.forfeit_stake(&id, &ana), STAKE);
    assert_eq!(s.usdc.balance(&funder), REWARD + STAKE);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

// ---------- TTL y custodia ----------

#[test]
fn la_garantia_vive_en_la_red_hasta_el_plazo_y_la_gracia_para_cobrarla() {
    let s = setup();
    let (_, id) = posted(&s);
    let ana = reviewer(&s);
    s.advance(DAY);
    s.splitter.accept_bounty(&ana, &id, &STAKE);
    let ttl = s.env.as_contract(&s.splitter.address, || {
        s.env
            .storage()
            .persistent()
            .get_ttl(&DataKey::Stake(id, ana.clone()))
    });
    assert_eq!(
        ttl,
        ((6 + REFUND_GRACE_DAYS) as u64 * DAY / 5) as u32 + 7 * DAY_LEDGERS
    );
}

#[test]
fn el_saldo_del_contrato_cuenta_las_recompensas_abiertas_las_garantias_y_lo_pendiente() {
    let s = setup_with(true);
    let (funder_a, first) = posted(&s);
    let (funder_b, second) = posted(&s);
    let ana = reviewer(&s);
    let beto = reviewer(&s);
    let caro = reviewer(&s);
    let check = || {
        let mut held = s.splitter.owed(&funder_a) + s.splitter.owed(&funder_b);
        for id in [first, second] {
            if s.splitter.bounty(&id).unwrap().open {
                held += REWARD;
            }
            for who in [&ana, &beto, &caro] {
                held += s.splitter.stake(&id, who);
            }
        }
        assert_eq!(s.usdc.balance(&s.splitter.address), held);
    };
    check();

    s.splitter.accept_bounty(&ana, &first, &STAKE);
    s.splitter.accept_bounty(&beto, &first, &(2 * STAKE));
    s.splitter.accept_bounty(&caro, &second, &STAKE);
    s.splitter.accept_bounty(&ana, &second, &STAKE);
    check();

    s.splitter.submit_review(&ana, &first, &report(&s.env));
    check();
    let winners = vec![&s.env, share(&ana, 10_000)];
    s.splitter.award_bounty(&funder_a, &first, &winners);
    check();

    s.advance(7 * DAY);
    let issuer = StellarAssetClient::new(&s.env, &s.usdc.address);
    issuer.set_authorized(&funder_a, &false);
    s.splitter.forfeit_stake(&first, &beto);
    check();
    assert_eq!(s.splitter.owed(&funder_a), 2 * STAKE);
    s.splitter.forfeit_stake(&second, &caro);
    check();
    s.splitter.cancel_bounty(&funder_b, &second);
    check();
    issuer.set_authorized(&funder_a, &true);
    s.splitter.claim(&funder_a);
    check();
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        STAKE,
        "queda la garantía de Ana en la segunda, sin cobrar"
    );
}
