//! Pago contra entrega (v0.13.0): `lock_payment`, `unlock_payment` y `reclaim_payment`.

use super::*;
use crate::{Lock, PaymentLocked, PaymentReclaimed, PaymentUnlocked, MAX_LOCK_DAYS};
use soroban_sdk::{Bytes, BytesN};

/// El plazo de entrega de prueba: una hora.
const HOUR: u64 = 3_600;

/// Una llave de 32 bytes, como la que abre un artículo cifrado.
fn key(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

/// El sha256 de una llave: lo que la editorial le da al lector junto con el artículo cifrado.
fn hash_of(env: &Env, key: &BytesN<32>) -> BytesN<32> {
    env.crypto().sha256(&Bytes::from(key.clone())).to_bytes()
}

/// Un pago de 0.50 de `payer` a `publisher`, bloqueado contra la llave 7 con una hora de plazo.
fn locked(s: &Setup) -> u32 {
    locked_for(s, &s.publisher)
}

fn locked_for(s: &Setup, publisher: &Address) -> u32 {
    s.splitter.lock_payment(
        &s.payer,
        publisher,
        &paper(&s.env),
        &PRICE,
        &hash_of(&s.env, &key(&s.env, 7)),
        &(s.now() + HOUR),
    )
}

// ---------- lock_payment ----------

#[test]
fn el_pago_queda_en_el_contrato_atado_al_hash_de_la_llave() {
    let s = setup();
    let hash = hash_of(&s.env, &key(&s.env, 7));
    let deadline = s.now() + HOUR;
    let before = s.usdc.balance(&s.payer);
    let id = s.splitter.lock_payment(
        &s.payer,
        &s.publisher,
        &paper(&s.env),
        &PRICE,
        &hash,
        &deadline,
    );
    let receipts = s.receipts();

    assert_eq!(id, 0);
    assert_eq!(s.usdc.balance(&s.payer), before - PRICE);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        PRICE,
        "el pago queda en el contrato"
    );
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(
        s.splitter.lock(&id),
        Some(Lock {
            reader: s.payer.clone(),
            publisher: s.publisher.clone(),
            paper_id: paper(&s.env),
            amount: PRICE,
            key_hash: hash.clone(),
            deadline,
            open: true,
        })
    );
    let expected = PaymentLocked {
        reader: s.payer.clone(),
        publisher: s.publisher.clone(),
        lock_id: 0,
        paper_id: paper(&s.env),
        amount: PRICE,
        key_hash: hash,
        deadline,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    // Los ids son secuenciales y cada pago guarda lo suyo.
    let second = s.splitter.lock_payment(
        &s.payer,
        &s.publisher,
        &paper(&s.env),
        &3_000_000,
        &hash_of(&s.env, &key(&s.env, 8)),
        &(deadline + 60),
    );
    assert_eq!(second, 1);
    assert_eq!(s.splitter.lock(&second).unwrap().amount, 3_000_000);
    assert_eq!(s.splitter.lock(&id).unwrap().amount, PRICE);
    assert_eq!(s.splitter.lock(&2), None);
    assert_eq!(s.usdc.balance(&s.splitter.address), PRICE + 3_000_000);
}

#[test]
fn bloquear_solo_pide_la_firma_de_quien_paga() {
    let s = setup();
    let hash = hash_of(&s.env, &key(&s.env, 7));
    let deadline = s.now() + HOUR;
    s.splitter.lock_payment(
        &s.payer,
        &s.publisher,
        &paper(&s.env),
        &PRICE,
        &hash,
        &deadline,
    );

    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.payer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "lock_payment"),
                    (
                        s.payer.clone(),
                        s.publisher.clone(),
                        paper(&s.env),
                        PRICE,
                        hash,
                        deadline
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(&s.payer, &s.splitter.address, PRICE)],
            }
        )]
    );
}

#[test]
fn bloquear_rechaza_monto_plazo_e_id_invalidos_y_sin_firma_no_hay_pago() {
    let s = setup();
    let hash = hash_of(&s.env, &key(&s.env, 7));
    let now = s.now();
    let max = MAX_LOCK_DAYS as u64 * DAY;
    let before = s.usdc.balance(&s.payer);

    for amount in [0, -1] {
        assert_eq!(
            s.splitter.try_lock_payment(
                &s.payer,
                &s.publisher,
                &paper(&s.env),
                &amount,
                &hash,
                &(now + HOUR)
            ),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    for deadline in [now, now - 1, now + max + 1] {
        assert_eq!(
            s.splitter.try_lock_payment(
                &s.payer,
                &s.publisher,
                &paper(&s.env),
                &PRICE,
                &hash,
                &deadline
            ),
            Err(Ok(Error::InvalidDeadline.into()))
        );
    }
    assert_eq!(
        s.splitter.try_lock_payment(
            &s.payer,
            &s.publisher,
            &String::from_str(&s.env, ""),
            &PRICE,
            &hash,
            &(now + HOUR)
        ),
        Err(Ok(Error::InvalidPaperId.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.splitter.lock(&0), None);

    // Los bordes del plazo sí valen: un segundo después de ahora y justo el máximo.
    for deadline in [now + 1, now + max] {
        assert!(s
            .splitter
            .try_lock_payment(
                &s.payer,
                &s.publisher,
                &paper(&s.env),
                &PRICE,
                &hash,
                &deadline
            )
            .is_ok());
    }

    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_lock_payment(
            &s.payer,
            &s.publisher,
            &paper(&s.env),
            &PRICE,
            &hash,
            &(now + HOUR)
        )
        .is_err());
    assert_eq!(s.usdc.balance(&s.payer), before - 2 * PRICE);
    assert_eq!(s.splitter.lock(&2), None);
}

// ---------- unlock_payment ----------

#[test]
fn con_la_llave_la_editorial_cobra_el_98_y_la_llave_queda_publicada() {
    let s = setup();
    let id = locked(&s);
    let split = s.splitter.unlock_payment(&id, &key(&s.env, 7));
    let receipts = s.receipts();

    assert_eq!(
        split,
        Split {
            publisher: 4_900_000,
            fee: 100_000
        }
    );
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert!(!s.splitter.lock(&id).unwrap().open);
    assert_eq!(
        receipts,
        [
            Paid {
                publisher: s.publisher.clone(),
                reader: s.payer.clone(),
                paper_id: paper(&s.env),
                payer: s.payer.clone(),
                amount: PRICE,
                fee: 100_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            PaymentUnlocked {
                reader: s.payer.clone(),
                publisher: s.publisher.clone(),
                lock_id: id,
                key: key(&s.env, 7),
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );
}

#[test]
fn liberar_no_pide_firma_la_llave_es_la_prueba() {
    let s = setup();
    let id = locked(&s);
    s.env.set_auths(&[]);
    s.splitter.unlock_payment(&id, &key(&s.env, 7));

    assert_eq!(s.env.auths(), std::vec![]);
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
}

#[test]
fn una_llave_que_no_es_la_pactada_no_libera_nada() {
    let s = setup();
    let id = locked(&s);
    let other = s.splitter.lock_payment(
        &s.payer,
        &s.publisher,
        &paper(&s.env),
        &3_000_000,
        &hash_of(&s.env, &key(&s.env, 8)),
        &(s.now() + HOUR),
    );

    assert_eq!(
        s.splitter.try_unlock_payment(&id, &key(&s.env, 8)),
        Err(Ok(Error::WrongKey.into()))
    );
    // El hash es público: presentarlo como si fuera la llave no sirve.
    assert_eq!(
        s.splitter
            .try_unlock_payment(&id, &hash_of(&s.env, &key(&s.env, 7))),
        Err(Ok(Error::WrongKey.into()))
    );
    // La llave de un pago no abre otro.
    assert_eq!(
        s.splitter.try_unlock_payment(&other, &key(&s.env, 7)),
        Err(Ok(Error::WrongKey.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), PRICE + 3_000_000);
    assert_eq!(s.usdc.balance(&s.publisher), 0);

    s.splitter.unlock_payment(&other, &key(&s.env, 8));
    assert_eq!(s.usdc.balance(&s.publisher), 2_940_000);
    assert!(s.splitter.lock(&id).unwrap().open);
    assert_eq!(s.usdc.balance(&s.splitter.address), PRICE);
}

#[test]
fn la_llave_solo_vale_antes_del_plazo() {
    let s = setup();
    let on_time = locked(&s);
    let late = locked(&s);

    s.advance(HOUR - 1);
    s.splitter.unlock_payment(&on_time, &key(&s.env, 7));
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);

    s.advance(1);
    assert_eq!(
        s.splitter.try_unlock_payment(&late, &key(&s.env, 7)),
        Err(Ok(Error::LockExpired.into()))
    );
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), PRICE);
    assert!(s.splitter.lock(&late).unwrap().open);
}

#[test]
fn una_editorial_sin_trustline_no_pierde_el_cobro_queda_pendiente() {
    let s = setup();
    let nueva = g_account(&s.env, 41);
    let id = locked_for(&s, &nueva);
    s.splitter.unlock_payment(&id, &key(&s.env, 7));
    let receipts = s.receipts();

    assert_eq!(s.splitter.owed(&nueva), 4_900_000);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        4_900_000,
        "lo pendiente sigue en el contrato"
    );
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
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
                reader: s.payer.clone(),
                paper_id: paper(&s.env),
                payer: s.payer.clone(),
                amount: PRICE,
                fee: 100_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            PaymentUnlocked {
                reader: s.payer.clone(),
                publisher: nueva.clone(),
                lock_id: id,
                key: key(&s.env, 7),
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );

    s.splitter.claim(&nueva);
    assert_eq!(s.usdc.balance(&nueva), 4_900_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

#[test]
fn no_se_libera_dos_veces_ni_un_pago_inexistente() {
    let s = setup();
    let id = locked(&s);

    assert_eq!(
        s.splitter.try_unlock_payment(&99, &key(&s.env, 7)),
        Err(Ok(Error::NoLock.into()))
    );
    s.splitter.unlock_payment(&id, &key(&s.env, 7));
    assert_eq!(
        s.splitter.try_unlock_payment(&id, &key(&s.env, 7)),
        Err(Ok(Error::LockClosed.into()))
    );
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

// ---------- reclaim_payment ----------

#[test]
fn sin_llave_a_tiempo_quien_pago_recupera_todo_sin_comision() {
    let s = setup();
    let before = s.usdc.balance(&s.payer);
    let id = locked(&s);
    s.advance(HOUR);
    let back = s.splitter.reclaim_payment(&s.payer, &id);
    let receipts = s.receipts();

    assert_eq!(back, PRICE);
    assert_eq!(s.usdc.balance(&s.payer), before);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert!(!s.splitter.lock(&id).unwrap().open);
    let expected = PaymentReclaimed {
        reader: s.payer.clone(),
        lock_id: id,
        amount: PRICE,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn no_se_recupera_antes_del_plazo_y_si_en_el_segundo_exacto() {
    let s = setup();
    let id = locked(&s);

    assert_eq!(
        s.splitter.try_reclaim_payment(&s.payer, &id),
        Err(Ok(Error::LockActive.into()))
    );
    s.advance(HOUR - 1);
    assert_eq!(
        s.splitter.try_reclaim_payment(&s.payer, &id),
        Err(Ok(Error::LockActive.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), PRICE);

    s.advance(1);
    assert_eq!(s.splitter.reclaim_payment(&s.payer, &id), PRICE);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

#[test]
fn solo_quien_pago_recupera_y_solo_con_su_firma() {
    let s = setup();
    let id = locked(&s);
    let stranger = Address::generate(&s.env);
    s.advance(HOUR);

    assert_eq!(
        s.splitter.try_reclaim_payment(&stranger, &id),
        Err(Ok(Error::NoLock.into()))
    );
    // Tampoco la editorial: sin la llave, ese dinero no es suyo.
    assert_eq!(
        s.splitter.try_reclaim_payment(&s.publisher, &id),
        Err(Ok(Error::NoLock.into()))
    );
    assert_eq!(
        s.splitter.try_reclaim_payment(&s.payer, &99),
        Err(Ok(Error::NoLock.into()))
    );
    s.env.set_auths(&[]);
    assert!(s.splitter.try_reclaim_payment(&s.payer, &id).is_err());
    assert_eq!(s.usdc.balance(&s.splitter.address), PRICE);

    s.env.mock_all_auths();
    s.splitter.reclaim_payment(&s.payer, &id);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.payer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "reclaim_payment"),
                    (s.payer.clone(), id).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
}

#[test]
fn un_pago_devuelto_no_se_libera_y_uno_liberado_no_se_devuelve() {
    let s = setup();
    let before = s.usdc.balance(&s.payer);
    let reclaimed = locked(&s);
    let unlocked = locked(&s);
    s.splitter.unlock_payment(&unlocked, &key(&s.env, 7));
    s.advance(HOUR);
    s.splitter.reclaim_payment(&s.payer, &reclaimed);

    assert_eq!(
        s.splitter.try_unlock_payment(&reclaimed, &key(&s.env, 7)),
        Err(Ok(Error::LockClosed.into()))
    );
    assert_eq!(
        s.splitter.try_reclaim_payment(&s.payer, &reclaimed),
        Err(Ok(Error::LockClosed.into()))
    );
    assert_eq!(
        s.splitter.try_reclaim_payment(&s.payer, &unlocked),
        Err(Ok(Error::LockClosed.into()))
    );
    assert_eq!(s.usdc.balance(&s.payer), before - PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

// ---------- TTL y custodia ----------

#[test]
fn el_pago_bloqueado_vive_en_la_red_hasta_su_plazo_y_la_gracia_para_recuperarlo() {
    let s = setup();
    let id = locked(&s);
    let ttl = s.env.as_contract(&s.splitter.address, || {
        s.env.storage().persistent().get_ttl(&DataKey::Lock(id))
    });
    assert_eq!(
        ttl,
        ((HOUR + REFUND_GRACE_DAYS as u64 * DAY) / 5) as u32 + 7 * DAY_LEDGERS
    );
}

#[test]
fn el_saldo_del_contrato_cuenta_los_pagos_bloqueados_las_recompensas_y_lo_pendiente() {
    let s = setup();
    let nueva = g_account(&s.env, 43);
    let funder = Address::generate(&s.env);
    s.fund(&funder, 10_000_000);
    let to_unlock = locked(&s);
    let to_reclaim = locked(&s);
    let stays = locked(&s);
    let to_pending = locked_for(&s, &nueva);
    let bounty = s
        .splitter
        .post_bounty(&funder, &paper(&s.env), &10_000_000, &(s.now() + DAY));
    let check = || {
        let mut held = s.splitter.owed(&nueva);
        for id in [to_unlock, to_reclaim, stays, to_pending] {
            let lock = s.splitter.lock(&id).unwrap();
            if lock.open {
                held += lock.amount;
            }
        }
        let posted = s.splitter.bounty(&bounty).unwrap();
        if posted.open {
            held += posted.amount;
        }
        assert_eq!(s.usdc.balance(&s.splitter.address), held);
    };
    check();

    s.splitter.unlock_payment(&to_unlock, &key(&s.env, 7));
    check();
    s.splitter.unlock_payment(&to_pending, &key(&s.env, 7));
    check();
    assert_eq!(s.splitter.owed(&nueva), 4_900_000);

    s.advance(HOUR);
    s.splitter.reclaim_payment(&s.payer, &to_reclaim);
    check();
    s.splitter.claim(&nueva);
    check();
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        PRICE + 10_000_000,
        "quedan un pago bloqueado y la recompensa"
    );
}
