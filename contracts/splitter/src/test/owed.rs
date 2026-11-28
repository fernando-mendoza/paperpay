//! Pagar con el saldo pendiente (v0.17.0): `pay_from_owed`.

use super::*;
use crate::PaidFromOwed;

/// La recompensa de prueba: 1 USDC.
const REWARD: i128 = 10_000_000;
/// Lo que queda pendiente de esa recompensa para quien la gana sin trustline: el 98%.
const EARNED: i128 = 9_800_000;

/// Una cuenta sin trustline de USDC que ganó una recompensa de 1 USDC: el contrato le debe 0.98.
fn earner(s: &Setup, seed: u8) -> Address {
    let funder = Address::generate(&s.env);
    s.fund(&funder, REWARD);
    let id = s
        .splitter
        .post_bounty(&funder, &paper(&s.env), &REWARD, &(s.now() + DAY));
    let winner = g_account(&s.env, seed);
    let winners = vec![&s.env, share(&winner, 10_000)];
    s.splitter.award_bounty(&funder, &id, &winners);
    winner
}

#[test]
fn quien_tiene_saldo_pendiente_paga_una_lectura_sin_trustline_y_sin_retirarlo() {
    let s = setup();
    let ana = earner(&s, 61);
    let treasury_before = s.usdc.balance(&s.treasury);
    assert_eq!(s.splitter.owed(&ana), EARNED);

    let split = s
        .splitter
        .pay_from_owed(&ana, &s.publisher, &paper(&s.env), &PRICE);
    let receipts = s.receipts();

    assert_eq!(
        split,
        Split {
            publisher: 4_900_000,
            fee: 100_000
        }
    );
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury) - treasury_before, 100_000);
    assert_eq!(s.splitter.owed(&ana), EARNED - PRICE);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        EARNED - PRICE,
        "el contrato guarda justo lo que le sigue debiendo"
    );
    assert_eq!(
        receipts,
        [
            Paid {
                publisher: s.publisher.clone(),
                reader: ana.clone(),
                paper_id: paper(&s.env),
                payer: ana.clone(),
                amount: PRICE,
                fee: 100_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            PaidFromOwed {
                account: ana.clone(),
                paper_id: paper(&s.env),
                amount: PRICE,
                remaining: EARNED - PRICE,
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );
}

#[test]
fn pagar_con_lo_pendiente_solo_pide_la_firma_de_esa_cuenta() {
    let s = setup();
    let ana = earner(&s, 61);
    s.splitter
        .pay_from_owed(&ana, &s.publisher, &paper(&s.env), &PRICE);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            ana.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "pay_from_owed"),
                    (ana.clone(), s.publisher.clone(), paper(&s.env), PRICE).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
}

#[test]
fn gastar_justo_lo_pendiente_lo_deja_en_cero() {
    let s = setup();
    let ana = earner(&s, 61);
    s.splitter
        .pay_from_owed(&ana, &s.publisher, &paper(&s.env), &EARNED);

    assert_eq!(s.splitter.owed(&ana), 0);
    let stored = s.env.as_contract(&s.splitter.address, || {
        s.env
            .storage()
            .persistent()
            .has(&DataKey::Owed(ana.clone()))
    });
    assert!(
        !stored,
        "no queda una entrada en cero pagando renta en la red"
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.usdc.balance(&s.publisher), 9_604_000);
    assert_eq!(
        s.splitter.try_claim(&ana),
        Err(Ok(Error::NothingToClaim.into()))
    );
    assert_eq!(
        s.splitter
            .try_pay_from_owed(&ana, &s.publisher, &paper(&s.env), &1),
        Err(Ok(Error::OwedTooLow.into()))
    );
}

#[test]
fn si_lo_pendiente_no_alcanza_no_se_mueve_nada() {
    let s = setup();
    let ana = earner(&s, 61);
    let nadie = Address::generate(&s.env);
    let treasury_before = s.usdc.balance(&s.treasury);

    assert_eq!(
        s.splitter
            .try_pay_from_owed(&ana, &s.publisher, &paper(&s.env), &(EARNED + 1)),
        Err(Ok(Error::OwedTooLow.into()))
    );
    // Una cuenta a la que el contrato no le debe nada, aunque tenga USDC en su wallet.
    assert_eq!(
        s.splitter
            .try_pay_from_owed(&s.payer, &s.publisher, &paper(&s.env), &PRICE),
        Err(Ok(Error::OwedTooLow.into()))
    );
    assert_eq!(
        s.splitter
            .try_pay_from_owed(&nadie, &s.publisher, &paper(&s.env), &1),
        Err(Ok(Error::OwedTooLow.into()))
    );
    assert_eq!(s.splitter.owed(&ana), EARNED);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(s.usdc.balance(&s.treasury), treasury_before);
    assert_eq!(s.usdc.balance(&s.splitter.address), EARNED);
}

#[test]
fn rechaza_montos_e_ids_invalidos_y_sin_firma_nadie_gasta_lo_pendiente_de_otro() {
    let s = setup();
    let ana = earner(&s, 61);

    for amount in [0, -1] {
        assert_eq!(
            s.splitter
                .try_pay_from_owed(&ana, &s.publisher, &paper(&s.env), &amount),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    assert_eq!(
        s.splitter
            .try_pay_from_owed(&ana, &s.publisher, &String::from_str(&s.env, ""), &PRICE),
        Err(Ok(Error::InvalidPaperId.into()))
    );
    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_pay_from_owed(&ana, &s.publisher, &paper(&s.env), &PRICE)
        .is_err());
    assert_eq!(s.splitter.owed(&ana), EARNED);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
}

#[test]
fn lo_que_sobra_se_cobra_despues_con_claim() {
    let s = setup();
    let ana = earner(&s, 61);
    s.splitter
        .pay_from_owed(&ana, &s.publisher, &paper(&s.env), &PRICE);
    s.splitter.pay_from_owed(
        &ana,
        &s.publisher,
        &String::from_str(&s.env, "quantum-cryptography-post-quantum"),
        &3_000_000,
    );

    assert_eq!(s.splitter.owed(&ana), 1_800_000);
    assert_eq!(s.splitter.claim(&ana), 1_800_000);
    assert_eq!(s.usdc.balance(&ana), 1_800_000);
    assert_eq!(s.splitter.owed(&ana), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000 + 2_940_000);
}

#[test]
fn si_la_editorial_no_puede_recibir_su_parte_queda_pendiente_a_su_nombre() {
    let s = setup();
    let ana = earner(&s, 61);
    let nueva = g_account(&s.env, 62);
    let treasury_before = s.usdc.balance(&s.treasury);
    s.splitter
        .pay_from_owed(&ana, &nueva, &paper(&s.env), &PRICE);
    let receipts = s.receipts();

    assert_eq!(s.splitter.owed(&nueva), 4_900_000);
    assert_eq!(s.splitter.owed(&ana), EARNED - PRICE);
    assert_eq!(s.usdc.balance(&s.treasury) - treasury_before, 100_000);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        EARNED - 100_000,
        "solo salió la comisión"
    );
    assert_eq!(
        receipts,
        [
            Credited {
                account: nueva.clone(),
                amount: 4_900_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            Paid {
                publisher: nueva.clone(),
                reader: ana.clone(),
                paper_id: paper(&s.env),
                payer: ana.clone(),
                amount: PRICE,
                fee: 100_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            PaidFromOwed {
                account: ana.clone(),
                paper_id: paper(&s.env),
                amount: PRICE,
                remaining: EARNED - PRICE,
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );

    // La editorial nueva también puede leer con eso, o cobrarlo.
    assert_eq!(s.splitter.claim(&nueva), 4_900_000);
    assert_eq!(s.usdc.balance(&nueva), 4_900_000);
}

#[test]
fn pagar_una_parte_no_acorta_la_vida_de_lo_que_queda_pendiente() {
    let s = setup();
    let ana = earner(&s, 61);
    s.advance(30 * DAY);
    s.splitter
        .pay_from_owed(&ana, &s.publisher, &paper(&s.env), &PRICE);
    let (ttl, max) = s.env.as_contract(&s.splitter.address, || {
        (
            s.env
                .storage()
                .persistent()
                .get_ttl(&DataKey::Owed(ana.clone())),
            s.env.storage().max_ttl(),
        )
    });
    assert_eq!(ttl, max);
}

#[test]
fn el_saldo_del_contrato_es_siempre_lo_pendiente_de_todas_las_cuentas() {
    let s = setup();
    let ana = earner(&s, 61);
    let beto = earner(&s, 63);
    let nueva = g_account(&s.env, 62);
    let check = || {
        assert_eq!(
            s.usdc.balance(&s.splitter.address),
            s.splitter.owed(&ana) + s.splitter.owed(&beto) + s.splitter.owed(&nueva)
        );
    };
    check();

    s.splitter
        .pay_from_owed(&ana, &s.publisher, &paper(&s.env), &PRICE);
    check();
    s.splitter
        .pay_from_owed(&beto, &nueva, &paper(&s.env), &PRICE);
    check();
    s.splitter
        .pay_from_owed(&nueva, &s.publisher, &paper(&s.env), &4_900_000);
    check();
    s.splitter.claim(&beto);
    check();
    s.splitter
        .pay_from_owed(&ana, &s.publisher, &paper(&s.env), &(EARNED - PRICE));
    check();
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}
