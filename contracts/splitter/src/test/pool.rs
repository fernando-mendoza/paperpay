//! Fondos de patrocinio (v0.7.0): `fund_pool`, `pay_sponsored` y `withdraw_pool`.

use super::*;
use crate::{Pool, PoolFunded, PoolWithdrawn, SponsoredRead};
use soroban_sdk::testutils::storage::Temporary as _;

/// Lo que deposita el patrocinador de prueba: 1.50 USDC (tres lecturas).
const DEPOSIT: i128 = 15_000_000;

struct Fund {
    sponsor: Address,
    operator: Address,
}

impl Setup<'_> {
    /// Un fondo de 1.50 para `self.publisher`, con tope de 0.50 por lectura.
    fn funded_pool(&self) -> Fund {
        let sponsor = Address::generate(&self.env);
        let operator = Address::generate(&self.env);
        self.fund(&sponsor, DEPOSIT);
        self.splitter
            .fund_pool(&sponsor, &self.publisher, &operator, &PRICE, &DEPOSIT);
        Fund { sponsor, operator }
    }

    fn pool_balance(&self, sponsor: &Address, publisher: &Address) -> i128 {
        self.splitter.pool(sponsor, publisher).unwrap().balance
    }
}

fn other_paper(env: &Env) -> String {
    String::from_str(env, "quantum-cryptography-post-quantum")
}

// ---------- fund_pool ----------

#[test]
fn un_deposito_queda_en_garantia_con_su_operador_y_su_tope() {
    let s = setup();
    let sponsor = Address::generate(&s.env);
    let operator = Address::generate(&s.env);
    s.fund(&sponsor, DEPOSIT);
    let balance = s
        .splitter
        .fund_pool(&sponsor, &s.publisher, &operator, &PRICE, &DEPOSIT);
    let receipts = s.receipts();

    assert_eq!(balance, DEPOSIT);
    assert_eq!(s.usdc.balance(&sponsor), 0);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        DEPOSIT,
        "el depósito queda en el contrato"
    );
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(
        s.splitter.pool(&sponsor, &s.publisher),
        Some(Pool {
            operator: operator.clone(),
            max_per_read: PRICE,
            balance: DEPOSIT,
        })
    );
    let expected = PoolFunded {
        sponsor: sponsor.clone(),
        publisher: s.publisher.clone(),
        operator,
        max_per_read: PRICE,
        amount: DEPOSIT,
        balance: DEPOSIT,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn una_recarga_se_suma_al_saldo_y_cambia_el_operador_y_el_tope() {
    let s = setup();
    let fund = s.funded_pool();
    let new_operator = Address::generate(&s.env);
    s.fund(&fund.sponsor, 2_000_000);
    let balance = s.splitter.fund_pool(
        &fund.sponsor,
        &s.publisher,
        &new_operator,
        &3_000_000,
        &2_000_000,
    );
    let receipts = s.receipts();

    assert_eq!(balance, DEPOSIT + 2_000_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), DEPOSIT + 2_000_000);
    assert_eq!(
        s.splitter.pool(&fund.sponsor, &s.publisher),
        Some(Pool {
            operator: new_operator.clone(),
            max_per_read: 3_000_000,
            balance: DEPOSIT + 2_000_000,
        })
    );
    let expected = PoolFunded {
        sponsor: fund.sponsor.clone(),
        publisher: s.publisher.clone(),
        operator: new_operator.clone(),
        max_per_read: 3_000_000,
        amount: 2_000_000,
        balance: DEPOSIT + 2_000_000,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    // El operador anterior ya no puede gastar; el nuevo sí, con el tope nuevo.
    let reader = Address::generate(&s.env);
    assert_eq!(
        s.splitter.try_pay_sponsored(
            &fund.operator,
            &fund.sponsor,
            &reader,
            &paper(&s.env),
            &s.publisher,
            &3_000_000
        ),
        Err(Ok(Error::NotOperator.into()))
    );
    assert_eq!(
        s.splitter.try_pay_sponsored(
            &new_operator,
            &fund.sponsor,
            &reader,
            &paper(&s.env),
            &s.publisher,
            &PRICE
        ),
        Err(Ok(Error::AboveMaxPerRead.into()))
    );
    s.splitter.pay_sponsored(
        &new_operator,
        &fund.sponsor,
        &reader,
        &paper(&s.env),
        &s.publisher,
        &3_000_000,
    );
}

#[test]
fn cada_fondo_es_de_un_patrocinador_y_una_editorial() {
    let s = setup();
    let fund = s.funded_pool();
    let other_publisher = Address::generate(&s.env);
    s.fund(&fund.sponsor, 4_000_000);
    s.splitter.fund_pool(
        &fund.sponsor,
        &other_publisher,
        &fund.operator,
        &PRICE,
        &4_000_000,
    );

    assert_eq!(s.pool_balance(&fund.sponsor, &s.publisher), DEPOSIT);
    assert_eq!(s.pool_balance(&fund.sponsor, &other_publisher), 4_000_000);
    assert_eq!(s.splitter.pool(&fund.operator, &s.publisher), None);
    assert_eq!(s.usdc.balance(&s.splitter.address), DEPOSIT + 4_000_000);
}

#[test]
fn el_deposito_solo_pide_la_firma_del_patrocinador() {
    let s = setup();
    let sponsor = Address::generate(&s.env);
    let operator = Address::generate(&s.env);
    s.fund(&sponsor, DEPOSIT);
    s.splitter
        .fund_pool(&sponsor, &s.publisher, &operator, &PRICE, &DEPOSIT);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            sponsor.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "fund_pool"),
                    (
                        sponsor.clone(),
                        s.publisher.clone(),
                        operator.clone(),
                        PRICE,
                        DEPOSIT
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(&sponsor, &s.splitter.address, DEPOSIT)],
            }
        )]
    );
}

#[test]
fn el_deposito_rechaza_montos_en_cero_y_sin_firma_no_entra_nada() {
    let s = setup();
    let sponsor = Address::generate(&s.env);
    let operator = Address::generate(&s.env);
    s.fund(&sponsor, DEPOSIT);

    for (max_per_read, amount) in [(PRICE, 0), (PRICE, -1), (0, DEPOSIT), (-1, DEPOSIT)] {
        assert_eq!(
            s.splitter
                .try_fund_pool(&sponsor, &s.publisher, &operator, &max_per_read, &amount),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    // Más de lo que tiene: falla la transferencia y no queda fondo.
    assert!(s
        .splitter
        .try_fund_pool(&sponsor, &s.publisher, &operator, &PRICE, &(DEPOSIT + 1))
        .is_err());

    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_fund_pool(&sponsor, &s.publisher, &operator, &PRICE, &DEPOSIT)
        .is_err());
    assert_eq!(s.usdc.balance(&sponsor), DEPOSIT);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.splitter.pool(&sponsor, &s.publisher), None);
}

// ---------- pay_sponsored ----------

#[test]
fn el_fondo_paga_la_lectura_de_quien_no_tiene_usdc_y_reparte_98_2() {
    let s = setup();
    let fund = s.funded_pool();
    let reader = Address::generate(&s.env);
    let split = s.splitter.pay_sponsored(
        &fund.operator,
        &fund.sponsor,
        &reader,
        &paper(&s.env),
        &s.publisher,
        &PRICE,
    );
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
    assert_eq!(s.usdc.balance(&reader), 0, "quien lee no paga ni recibe");
    assert_eq!(s.usdc.balance(&fund.operator), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), DEPOSIT - PRICE);
    assert_eq!(s.pool_balance(&fund.sponsor, &s.publisher), DEPOSIT - PRICE);
    assert_eq!(
        receipts,
        [
            Paid {
                publisher: s.publisher.clone(),
                reader: reader.clone(),
                paper_id: paper(&s.env),
                payer: fund.sponsor.clone(),
                amount: PRICE,
                fee: 100_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            SponsoredRead {
                sponsor: fund.sponsor.clone(),
                reader: reader.clone(),
                paper_id: paper(&s.env),
                publisher: s.publisher.clone(),
                amount: PRICE,
                balance: DEPOSIT - PRICE,
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );
}

#[test]
fn la_lectura_patrocinada_solo_pide_la_firma_del_operador() {
    let s = setup();
    let fund = s.funded_pool();
    let reader = Address::generate(&s.env);
    s.splitter.pay_sponsored(
        &fund.operator,
        &fund.sponsor,
        &reader,
        &paper(&s.env),
        &s.publisher,
        &PRICE,
    );

    assert_eq!(
        s.env.auths(),
        std::vec![(
            fund.operator.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "pay_sponsored"),
                    (
                        fund.operator.clone(),
                        fund.sponsor.clone(),
                        reader.clone(),
                        paper(&s.env),
                        s.publisher.clone(),
                        PRICE
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
}

#[test]
fn solo_el_operador_del_fondo_puede_gastarlo_y_solo_en_su_editorial() {
    let s = setup();
    let fund = s.funded_pool();
    let reader = Address::generate(&s.env);
    let stranger = Address::generate(&s.env);
    let other_publisher = Address::generate(&s.env);

    // Ni un extraño, ni el lector, ni el propio patrocinador: solo el operador.
    for signer in [&stranger, &reader, &fund.sponsor] {
        assert_eq!(
            s.splitter.try_pay_sponsored(
                signer,
                &fund.sponsor,
                &reader,
                &paper(&s.env),
                &s.publisher,
                &PRICE
            ),
            Err(Ok(Error::NotOperator.into()))
        );
    }
    // El fondo es para una editorial: no hay fondo de ese patrocinador para otra.
    assert_eq!(
        s.splitter.try_pay_sponsored(
            &fund.operator,
            &fund.sponsor,
            &reader,
            &paper(&s.env),
            &other_publisher,
            &PRICE
        ),
        Err(Ok(Error::NoPool.into()))
    );
    assert_eq!(
        s.splitter.try_pay_sponsored(
            &fund.operator,
            &stranger,
            &reader,
            &paper(&s.env),
            &s.publisher,
            &PRICE
        ),
        Err(Ok(Error::NoPool.into()))
    );

    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_pay_sponsored(
            &fund.operator,
            &fund.sponsor,
            &reader,
            &paper(&s.env),
            &s.publisher,
            &PRICE
        )
        .is_err());
    assert_eq!(s.pool_balance(&fund.sponsor, &s.publisher), DEPOSIT);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(s.usdc.balance(&other_publisher), 0);
}

#[test]
fn el_fondo_no_paga_mas_que_su_tope_ni_mas_de_lo_que_le_queda() {
    let s = setup();
    let fund = s.funded_pool();
    let reader = Address::generate(&s.env);
    let pay = |paper_id: &str, amount: i128| {
        s.splitter.try_pay_sponsored(
            &fund.operator,
            &fund.sponsor,
            &reader,
            &String::from_str(&s.env, paper_id),
            &s.publisher,
            &amount,
        )
    };

    assert_eq!(pay("a", PRICE + 1), Err(Ok(Error::AboveMaxPerRead.into())));
    assert_eq!(pay("a", 0), Err(Ok(Error::InvalidAmount.into())));
    assert_eq!(pay("a", -PRICE), Err(Ok(Error::InvalidAmount.into())));
    assert_eq!(pay("", PRICE), Err(Ok(Error::InvalidPaperId.into())));
    assert_eq!(s.pool_balance(&fund.sponsor, &s.publisher), DEPOSIT);

    // Justo el tope se paga, y el fondo alcanza exactamente para tres lecturas.
    assert!(pay("a", PRICE).is_ok());
    assert!(pay("b", PRICE).is_ok());
    assert!(pay("c", PRICE).is_ok());
    assert_eq!(s.pool_balance(&fund.sponsor, &s.publisher), 0);
    assert_eq!(pay("d", 1), Err(Ok(Error::PoolTooLow.into())));
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.usdc.balance(&s.publisher), 3 * 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 3 * 100_000);
}

#[test]
fn el_fondo_paga_una_vez_por_articulo_y_lector_cada_24_horas() {
    let s = setup();
    let fund = s.funded_pool();
    let ana = Address::generate(&s.env);
    let beto = Address::generate(&s.env);
    let pay = |reader: &Address, paper_id: &String| {
        s.splitter.try_pay_sponsored(
            &fund.operator,
            &fund.sponsor,
            reader,
            paper_id,
            &s.publisher,
            &2_000_000,
        )
    };

    assert!(pay(&ana, &paper(&s.env)).is_ok());
    assert_eq!(
        pay(&ana, &paper(&s.env)),
        Err(Ok(Error::AlreadySponsored.into()))
    );
    // Otro artículo u otro lector sí.
    assert!(pay(&ana, &other_paper(&s.env)).is_ok());
    assert!(pay(&beto, &paper(&s.env)).is_ok());
    assert_eq!(
        s.pool_balance(&fund.sponsor, &s.publisher),
        DEPOSIT - 6_000_000
    );

    // El marcador dura un día de ledgers: en su último ledger sigue ahí; en el siguiente, ya no.
    s.advance(DAY);
    assert_eq!(
        pay(&ana, &paper(&s.env)),
        Err(Ok(Error::AlreadySponsored.into()))
    );
    s.advance(5);
    assert!(pay(&ana, &paper(&s.env)).is_ok());
    assert_eq!(
        s.pool_balance(&fund.sponsor, &s.publisher),
        DEPOSIT - 8_000_000
    );
}

#[test]
fn si_la_editorial_no_puede_recibir_la_lectura_falla_y_el_fondo_queda_intacto() {
    let s = setup();
    let nueva = g_account(&s.env, 21);
    let sponsor = Address::generate(&s.env);
    let operator = Address::generate(&s.env);
    let reader = Address::generate(&s.env);
    s.fund(&sponsor, DEPOSIT);
    s.splitter
        .fund_pool(&sponsor, &nueva, &operator, &PRICE, &DEPOSIT);

    assert!(s
        .splitter
        .try_pay_sponsored(&operator, &sponsor, &reader, &paper(&s.env), &nueva, &PRICE)
        .is_err());
    assert_eq!(s.pool_balance(&sponsor, &nueva), DEPOSIT);
    assert_eq!(s.usdc.balance(&s.splitter.address), DEPOSIT);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(
        s.splitter.owed(&nueva),
        0,
        "una lectura patrocinada no deja nada pendiente"
    );
    let marked = s.env.as_contract(&s.splitter.address, || {
        s.env.storage().temporary().has(&DataKey::Sponsored(
            sponsor.clone(),
            reader.clone(),
            paper(&s.env),
        ))
    });
    assert!(!marked, "sin pago no queda marcador");

    // El patrocinador recupera todo.
    assert_eq!(s.splitter.withdraw_pool(&sponsor, &nueva, &DEPOSIT), 0);
    assert_eq!(s.usdc.balance(&sponsor), DEPOSIT);
}

// ---------- withdraw_pool ----------

#[test]
fn el_patrocinador_retira_una_parte_o_todo_lo_que_no_se_uso() {
    let s = setup();
    let fund = s.funded_pool();
    s.splitter.pay_sponsored(
        &fund.operator,
        &fund.sponsor,
        &Address::generate(&s.env),
        &paper(&s.env),
        &s.publisher,
        &PRICE,
    );

    let left = s
        .splitter
        .withdraw_pool(&fund.sponsor, &s.publisher, &4_000_000);
    let receipts = s.receipts();
    assert_eq!(left, 6_000_000);
    assert_eq!(s.usdc.balance(&fund.sponsor), 4_000_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 6_000_000);
    assert_eq!(s.pool_balance(&fund.sponsor, &s.publisher), 6_000_000);
    let expected = PoolWithdrawn {
        sponsor: fund.sponsor.clone(),
        publisher: s.publisher.clone(),
        amount: 4_000_000,
        balance: 6_000_000,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    assert_eq!(
        s.splitter
            .withdraw_pool(&fund.sponsor, &s.publisher, &6_000_000),
        0
    );
    assert_eq!(s.usdc.balance(&fund.sponsor), DEPOSIT - PRICE);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    // Un fondo vacío ya no paga, y se puede volver a cargar.
    assert_eq!(
        s.splitter.try_pay_sponsored(
            &fund.operator,
            &fund.sponsor,
            &Address::generate(&s.env),
            &paper(&s.env),
            &s.publisher,
            &PRICE
        ),
        Err(Ok(Error::PoolTooLow.into()))
    );
    assert_eq!(
        s.splitter
            .fund_pool(&fund.sponsor, &s.publisher, &fund.operator, &PRICE, &PRICE),
        PRICE
    );
}

#[test]
fn el_retiro_solo_pide_la_firma_del_patrocinador_y_el_operador_no_retira() {
    let s = setup();
    let fund = s.funded_pool();
    s.splitter
        .withdraw_pool(&fund.sponsor, &s.publisher, &PRICE);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            fund.sponsor.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "withdraw_pool"),
                    (fund.sponsor.clone(), s.publisher.clone(), PRICE).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );

    // El operador gasta en lecturas, pero no retira: a su nombre no hay fondo.
    assert_eq!(
        s.splitter
            .try_withdraw_pool(&fund.operator, &s.publisher, &PRICE),
        Err(Ok(Error::NoPool.into()))
    );
    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_withdraw_pool(&fund.sponsor, &s.publisher, &PRICE)
        .is_err());
    assert_eq!(s.usdc.balance(&fund.sponsor), PRICE);
    assert_eq!(s.usdc.balance(&fund.operator), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), DEPOSIT - PRICE);
}

#[test]
fn el_retiro_rechaza_mas_que_el_saldo_cero_y_fondos_que_no_existen() {
    let s = setup();
    let fund = s.funded_pool();
    let other_publisher = Address::generate(&s.env);

    assert_eq!(
        s.splitter
            .try_withdraw_pool(&fund.sponsor, &s.publisher, &(DEPOSIT + 1)),
        Err(Ok(Error::PoolTooLow.into()))
    );
    for amount in [0, -1] {
        assert_eq!(
            s.splitter
                .try_withdraw_pool(&fund.sponsor, &s.publisher, &amount),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    assert_eq!(
        s.splitter
            .try_withdraw_pool(&fund.sponsor, &other_publisher, &PRICE),
        Err(Ok(Error::NoPool.into()))
    );
    assert_eq!(s.pool_balance(&fund.sponsor, &s.publisher), DEPOSIT);
    assert_eq!(s.usdc.balance(&fund.sponsor), 0);
}

// ---------- TTL y custodia ----------

#[test]
fn el_fondo_vive_el_ttl_maximo_y_el_marcador_un_dia() {
    let s = setup();
    let fund = s.funded_pool();
    let reader = Address::generate(&s.env);
    let pool_key = DataKey::Pool(fund.sponsor.clone(), s.publisher.clone());
    let pool_ttl = || {
        s.env.as_contract(&s.splitter.address, || {
            s.env.storage().persistent().get_ttl(&pool_key)
        })
    };
    // `max_ttl()` es el máximo de la red menos 1: el ledger actual cuenta.
    assert_eq!(pool_ttl(), 3_110_400 - 1);

    s.advance(30 * DAY);
    s.splitter.pay_sponsored(
        &fund.operator,
        &fund.sponsor,
        &reader,
        &paper(&s.env),
        &s.publisher,
        &PRICE,
    );
    assert_eq!(pool_ttl(), 3_110_400 - 1, "cada lectura renueva el fondo");
    let marker_ttl = s.env.as_contract(&s.splitter.address, || {
        s.env.storage().temporary().get_ttl(&DataKey::Sponsored(
            fund.sponsor.clone(),
            reader.clone(),
            paper(&s.env),
        ))
    });
    assert_eq!(marker_ttl, DAY_LEDGERS);

    s.advance(30 * DAY);
    s.splitter
        .withdraw_pool(&fund.sponsor, &s.publisher, &PRICE);
    assert_eq!(pool_ttl(), 3_110_400 - 1, "cada retiro también");
}

/// USDC del contrato = campañas sin liberar + pendientes de cobro + saldos de los fondos.
fn assert_custody_with_pools(
    s: &Setup,
    campaigns: &[u32],
    accounts: &[&Address],
    pools: &[(&Address, &Address)],
) {
    let mut held: i128 = 0;
    for id in campaigns {
        let campaign = s.splitter.campaign(id).unwrap();
        if !campaign.released {
            held += campaign.raised;
        }
    }
    for account in accounts {
        held += s.splitter.owed(account);
    }
    for (sponsor, publisher) in pools {
        if let Some(pool) = s.splitter.pool(sponsor, publisher) {
            held += pool.balance;
        }
    }
    assert_eq!(s.usdc.balance(&s.splitter.address), held);
}

#[test]
fn el_saldo_del_contrato_es_campanas_mas_pendientes_mas_fondos() {
    let s = setup();
    let nueva = g_account(&s.env, 23);
    let ana = s.backer(GOAL);
    let fund = s.funded_pool();
    let other_publisher = Address::generate(&s.env);
    let accounts = [&nueva, &ana, &s.publisher, &s.treasury, &fund.sponsor];
    let pools = [
        (&fund.sponsor, &s.publisher),
        (&fund.sponsor, &other_publisher),
        (&ana, &s.publisher),
    ];
    let check = |ids: &[u32]| assert_custody_with_pools(&s, ids, &accounts, &pools);

    let to_new = s
        .splitter
        .open_campaign(&nueva, &paper(&s.env), &GOAL, &(s.now() + DAY));
    let fails = s.open_campaign();
    let ids = [to_new, fails];
    check(&ids);

    // Un segundo fondo del mismo patrocinador y un fondo propio de Ana (se paga sus lecturas).
    s.fund(&fund.sponsor, 4_000_000);
    s.splitter.fund_pool(
        &fund.sponsor,
        &other_publisher,
        &fund.operator,
        &PRICE,
        &4_000_000,
    );
    s.splitter
        .fund_pool(&ana, &s.publisher, &fund.operator, &PRICE, &3_000_000);
    s.splitter.pledge(&ana, &fails, &2_000_000);
    s.splitter.pledge(&s.backer(GOAL), &to_new, &GOAL);
    check(&ids);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        DEPOSIT + 4_000_000 + 3_000_000 + 2_000_000 + GOAL
    );

    s.splitter.pay_sponsored(
        &fund.operator,
        &fund.sponsor,
        &ana,
        &paper(&s.env),
        &s.publisher,
        &PRICE,
    );
    check(&ids);
    s.splitter.release_pledges(&to_new);
    check(&ids);
    s.splitter.pay_sponsored(
        &fund.operator,
        &ana,
        &ana,
        &other_paper(&s.env),
        &s.publisher,
        &2_000_000,
    );
    s.splitter
        .withdraw_pool(&fund.sponsor, &other_publisher, &1_000_000);
    check(&ids);

    s.advance(7 * DAY);
    s.splitter.refund_pledge(&fails, &ana);
    s.splitter.claim(&nueva);
    check(&ids);
    s.splitter
        .withdraw_pool(&fund.sponsor, &s.publisher, &(DEPOSIT - PRICE));
    s.splitter
        .withdraw_pool(&fund.sponsor, &other_publisher, &3_000_000);
    s.splitter.withdraw_pool(&ana, &s.publisher, &1_000_000);
    check(&ids);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);

    // Nadie perdió nada: cada peso fue a una editorial (98/2) o volvió a su dueño.
    assert_eq!(s.usdc.balance(&fund.sponsor), DEPOSIT + 4_000_000 - PRICE);
    assert_eq!(s.usdc.balance(&ana), GOAL - 2_000_000);
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000 + 1_960_000);
    assert_eq!(s.usdc.balance(&nueva), 9_800_000);
    assert_eq!(s.usdc.balance(&s.treasury), 200_000 + 100_000 + 40_000);
}
