//! Agente con tope (v0.16.0): `pay_as_agent`, con `authorize_agent` y `revoke_agent` de soporte.

use super::*;
use crate::{AgentAuthorized, AgentPaid, AgentPermit, AgentRevoked, MAX_AGENT_DAYS};

/// Tope por lectura de prueba: 0.50 USDC.
const CAP: i128 = PRICE;
/// Presupuesto de prueba: 2 USDC, cuatro lecturas de 0.50.
const BUDGET: i128 = 20_000_000;

/// `payer` autoriza a un agente nuevo: hasta 0.50 por lectura, 2.00 en total, 7 días.
fn authorized(s: &Setup) -> Address {
    let agent = Address::generate(&s.env);
    s.splitter
        .authorize_agent(&s.payer, &agent, &CAP, &BUDGET, &7);
    agent
}

fn allowance(s: &Setup, owner: &Address) -> i128 {
    s.usdc.allowance(owner, &s.splitter.address)
}

// ---------- authorize_agent (soporte) ----------

#[test]
fn autorizar_un_agente_no_mueve_fondos_y_fija_su_presupuesto_en_el_token() {
    let s = setup();
    let agent = Address::generate(&s.env);
    let before = s.usdc.balance(&s.payer);
    let expires = s
        .splitter
        .authorize_agent(&s.payer, &agent, &CAP, &BUDGET, &7);
    let receipts = s.receipts();

    assert_eq!(expires, START + 7 * DAY);
    assert_eq!(s.usdc.balance(&s.payer), before, "no se mueve nada");
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(allowance(&s, &s.payer), BUDGET);
    assert_eq!(
        s.splitter.agent(&s.payer),
        Some(AgentPermit {
            agent: agent.clone(),
            max_per_read: CAP,
            expires_at: expires,
        })
    );
    let expected = AgentAuthorized {
        owner: s.payer.clone(),
        agent: agent.clone(),
        max_per_read: CAP,
        budget: BUDGET,
        expires_at: expires,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn autorizar_pide_la_firma_del_dueno_que_cubre_el_presupuesto_en_el_token() {
    let s = setup();
    let agent = Address::generate(&s.env);
    let seq = s.env.ledger().sequence();
    s.splitter
        .authorize_agent(&s.payer, &agent, &CAP, &BUDGET, &7);
    let live_until = (seq / DAY_LEDGERS + 7 + 2) * DAY_LEDGERS;

    assert_eq!(
        s.env.auths(),
        std::vec![(
            s.payer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "authorize_agent"),
                    (s.payer.clone(), agent.clone(), CAP, BUDGET, 7u32).into_val(&s.env),
                )),
                sub_invocations: std::vec![AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        s.usdc.address.clone(),
                        Symbol::new(&s.env, "approve"),
                        (
                            s.payer.clone(),
                            s.splitter.address.clone(),
                            BUDGET,
                            live_until
                        )
                            .into_val(&s.env),
                    )),
                    sub_invocations: std::vec![],
                }],
            }
        )]
    );
}

#[test]
fn autorizar_rechaza_topes_presupuestos_y_dias_invalidos() {
    let s = setup();
    let agent = Address::generate(&s.env);

    for cap in [0, -1] {
        assert_eq!(
            s.splitter
                .try_authorize_agent(&s.payer, &agent, &cap, &BUDGET, &7),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    // El presupuesto alcanza al menos para una lectura al tope.
    assert_eq!(
        s.splitter
            .try_authorize_agent(&s.payer, &agent, &CAP, &(CAP - 1), &7),
        Err(Ok(Error::InvalidAmount.into()))
    );
    for days in [0, MAX_AGENT_DAYS + 1] {
        assert_eq!(
            s.splitter
                .try_authorize_agent(&s.payer, &agent, &CAP, &BUDGET, &days),
            Err(Ok(Error::InvalidDays.into()))
        );
    }
    assert_eq!(s.splitter.agent(&s.payer), None);
    assert_eq!(allowance(&s, &s.payer), 0);

    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_authorize_agent(&s.payer, &agent, &CAP, &BUDGET, &7)
        .is_err());
    assert_eq!(s.splitter.agent(&s.payer), None);

    // Los bordes sí valen: presupuesto igual al tope y el máximo de días.
    s.env.mock_all_auths();
    assert_eq!(
        s.splitter
            .authorize_agent(&s.payer, &agent, &CAP, &CAP, &MAX_AGENT_DAYS),
        START + MAX_AGENT_DAYS as u64 * DAY
    );
}

#[test]
fn con_una_suscripcion_activa_no_se_autoriza_un_agente_comparten_la_autorizacion_del_token() {
    let s = setup();
    let agent = Address::generate(&s.env);
    s.fund(&s.payer, pass_price(40));
    s.splitter
        .subscribe(&s.payer, &s.publisher, &7, &3, &pass_price(7));
    let reserved = allowance(&s, &s.payer);

    assert_eq!(
        s.splitter
            .try_authorize_agent(&s.payer, &agent, &CAP, &BUDGET, &7),
        Err(Ok(Error::AllowanceInUse.into()))
    );
    assert_eq!(
        allowance(&s, &s.payer),
        reserved,
        "la de la suscripción sigue intacta"
    );
    assert_eq!(s.splitter.agent(&s.payer), None);
}

// ---------- pay_as_agent ----------

#[test]
fn el_agente_paga_una_lectura_con_el_usdc_de_su_dueno_y_el_acceso_es_del_dueno() {
    let s = setup();
    let agent = authorized(&s);
    let before = s.usdc.balance(&s.payer);
    let split = s
        .splitter
        .pay_as_agent(&agent, &s.payer, &s.publisher, &paper(&s.env), &PRICE);
    let receipts = s.receipts();

    assert_eq!(
        split,
        Split {
            publisher: 4_900_000,
            fee: 100_000
        }
    );
    assert_eq!(s.usdc.balance(&s.payer), before - PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(s.usdc.balance(&agent), 0, "el agente no toca el dinero");
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        0,
        "el contrato tampoco"
    );
    assert_eq!(allowance(&s, &s.payer), BUDGET - PRICE);
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
            AgentPaid {
                owner: s.payer.clone(),
                agent: agent.clone(),
                paper_id: paper(&s.env),
                amount: PRICE,
                remaining: BUDGET - PRICE,
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );
}

#[test]
fn el_pago_del_agente_solo_pide_la_firma_del_agente() {
    let s = setup();
    let agent = authorized(&s);
    let args: soroban_sdk::Vec<soroban_sdk::Val> = (
        agent.clone(),
        s.payer.clone(),
        s.publisher.clone(),
        paper(&s.env),
        PRICE,
    )
        .into_val(&s.env);
    // Solo vale la firma del agente sobre esa llamada: ni el dueño ni nadie más firma.
    s.env.mock_auths(&[MockAuth {
        address: &agent,
        invoke: &MockAuthInvoke {
            contract: &s.splitter.address,
            fn_name: "pay_as_agent",
            args: args.clone(),
            sub_invokes: &[],
        },
    }]);
    s.splitter
        .pay_as_agent(&agent, &s.payer, &s.publisher, &paper(&s.env), &PRICE);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            agent.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "pay_as_agent"),
                    args,
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
}

#[test]
fn el_agente_no_pasa_del_tope_por_lectura_ni_del_presupuesto() {
    let s = setup();
    let agent = authorized(&s);
    let before = s.usdc.balance(&s.payer);
    let pay = |amount: i128| {
        s.splitter
            .try_pay_as_agent(&agent, &s.payer, &s.publisher, &paper(&s.env), &amount)
    };

    assert_eq!(pay(CAP + 1), Err(Ok(Error::AboveMaxPerRead.into())));
    for amount in [0, -1] {
        assert_eq!(pay(amount), Err(Ok(Error::InvalidAmount.into())));
    }
    assert_eq!(
        s.splitter.try_pay_as_agent(
            &agent,
            &s.payer,
            &s.publisher,
            &String::from_str(&s.env, ""),
            &PRICE
        ),
        Err(Ok(Error::InvalidPaperId.into()))
    );
    assert_eq!(s.usdc.balance(&s.payer), before);

    // El presupuesto de 2.00 alcanza para tres lecturas al tope y una de 0.40; quedan 0.10.
    for _ in 0..3 {
        assert!(pay(CAP).is_ok());
    }
    assert!(pay(4_000_000).is_ok());
    assert_eq!(allowance(&s, &s.payer), 1_000_000);
    assert_eq!(pay(1_000_001), Err(Ok(Error::BudgetExhausted.into())));
    assert!(pay(1_000_000).is_ok(), "justo lo que queda sí se puede");
    assert_eq!(pay(1), Err(Ok(Error::BudgetExhausted.into())));
    assert_eq!(s.usdc.balance(&s.payer), before - BUDGET);
    assert_eq!(
        s.usdc.balance(&s.publisher) + s.usdc.balance(&s.treasury),
        BUDGET
    );
    assert_eq!(allowance(&s, &s.payer), 0);
}

#[test]
fn solo_paga_el_agente_autorizado_y_solo_por_su_dueno() {
    let s = setup();
    let agent = authorized(&s);
    let stranger = Address::generate(&s.env);
    let other_owner = Address::generate(&s.env);
    s.fund(&other_owner, 10 * PRICE);
    let before = s.usdc.balance(&s.payer);
    let pay = |who: &Address, owner: &Address| {
        s.splitter
            .try_pay_as_agent(who, owner, &s.publisher, &paper(&s.env), &PRICE)
    };

    assert_eq!(pay(&stranger, &s.payer), Err(Ok(Error::NoAgent.into())));
    // El agente de una cuenta no puede gastar el dinero de otra.
    assert_eq!(pay(&agent, &other_owner), Err(Ok(Error::NoAgent.into())));
    // La editorial tampoco se cobra sola.
    assert_eq!(pay(&s.publisher, &s.payer), Err(Ok(Error::NoAgent.into())));

    s.env.set_auths(&[]);
    assert!(pay(&agent, &s.payer).is_err());
    assert_eq!(s.usdc.balance(&s.payer), before);
    assert_eq!(s.usdc.balance(&other_owner), 10 * PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(allowance(&s, &s.payer), BUDGET);
}

#[test]
fn el_permiso_vence_y_el_agente_ya_no_paga() {
    let s = setup();
    let agent = authorized(&s);
    let pay = || {
        s.splitter
            .try_pay_as_agent(&agent, &s.payer, &s.publisher, &paper(&s.env), &PRICE)
    };

    s.advance(7 * DAY - 1);
    assert!(pay().is_ok(), "un segundo antes todavía paga");
    s.advance(1);
    assert_eq!(pay(), Err(Ok(Error::AgentExpired.into())));
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
}

#[test]
fn sin_saldo_del_dueno_el_pago_del_agente_falla_y_no_cambia_nada() {
    let s = setup();
    let broke = Address::generate(&s.env);
    let agent = Address::generate(&s.env);
    s.splitter
        .authorize_agent(&broke, &agent, &CAP, &BUDGET, &7);

    assert!(s
        .splitter
        .try_pay_as_agent(&agent, &broke, &s.publisher, &paper(&s.env), &PRICE)
        .is_err());
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);
    assert_eq!(allowance(&s, &broke), BUDGET);
}

#[test]
fn si_el_dueno_se_suscribe_despues_el_agente_no_gasta_la_autorizacion_de_la_suscripcion() {
    let s = setup();
    let agent = authorized(&s);
    s.fund(&s.payer, pass_price(40));
    s.splitter
        .subscribe(&s.payer, &s.publisher, &7, &3, &pass_price(7));
    let reserved = allowance(&s, &s.payer);
    let publisher_before = s.usdc.balance(&s.publisher);

    assert_eq!(
        s.splitter
            .try_pay_as_agent(&agent, &s.payer, &s.publisher, &paper(&s.env), &PRICE),
        Err(Ok(Error::AllowanceInUse.into()))
    );
    assert_eq!(allowance(&s, &s.payer), reserved);
    assert_eq!(s.usdc.balance(&s.publisher), publisher_before);

    // Revocar el permiso tampoco toca la autorización, que ahora es de la suscripción. Y sigue
    // pidiendo la firma del dueño, aunque aquí no haya ninguna llamada al token que la exija.
    s.env.set_auths(&[]);
    assert!(s.splitter.try_revoke_agent(&s.payer).is_err());
    assert!(s.splitter.agent(&s.payer).is_some());
    s.env.mock_all_auths();
    s.splitter.revoke_agent(&s.payer);
    assert_eq!(s.splitter.agent(&s.payer), None);
    assert_eq!(allowance(&s, &s.payer), reserved);
}

// ---------- revoke_agent (soporte) y reemplazo ----------

#[test]
fn revocar_borra_el_permiso_y_deja_en_cero_la_autorizacion_del_token() {
    let s = setup();
    let agent = authorized(&s);
    s.splitter.revoke_agent(&s.payer);
    let receipts = s.receipts();

    assert_eq!(s.splitter.agent(&s.payer), None);
    assert_eq!(allowance(&s, &s.payer), 0);
    let expected = AgentRevoked {
        owner: s.payer.clone(),
        agent: agent.clone(),
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
    assert_eq!(
        s.splitter
            .try_pay_as_agent(&agent, &s.payer, &s.publisher, &paper(&s.env), &PRICE),
        Err(Ok(Error::NoAgent.into()))
    );
    assert_eq!(
        s.splitter.try_revoke_agent(&s.payer),
        Err(Ok(Error::NoAgent.into()))
    );

    // Solo el dueño revoca: sin su firma, el permiso sigue.
    let again = authorized(&s);
    s.env.set_auths(&[]);
    assert!(s.splitter.try_revoke_agent(&s.payer).is_err());
    assert_eq!(s.splitter.agent(&s.payer).unwrap().agent, again);
}

#[test]
fn autorizar_de_nuevo_reemplaza_al_agente_y_su_presupuesto() {
    let s = setup();
    let old = authorized(&s);
    s.splitter
        .pay_as_agent(&old, &s.payer, &s.publisher, &paper(&s.env), &PRICE);
    let new = Address::generate(&s.env);
    s.splitter
        .authorize_agent(&s.payer, &new, &3_000_000, &6_000_000, &1);

    assert_eq!(
        allowance(&s, &s.payer),
        6_000_000,
        "el presupuesto nuevo, no la suma"
    );
    assert_eq!(
        s.splitter
            .try_pay_as_agent(&old, &s.payer, &s.publisher, &paper(&s.env), &PRICE),
        Err(Ok(Error::NoAgent.into()))
    );
    assert_eq!(
        s.splitter
            .try_pay_as_agent(&new, &s.payer, &s.publisher, &paper(&s.env), &PRICE),
        Err(Ok(Error::AboveMaxPerRead.into()))
    );
    s.splitter
        .pay_as_agent(&new, &s.payer, &s.publisher, &paper(&s.env), &3_000_000);
    assert_eq!(allowance(&s, &s.payer), 3_000_000);
    assert_eq!(s.splitter.agent(&s.payer).unwrap().expires_at, START + DAY);
}

// ---------- TTL ----------

#[test]
fn el_permiso_vive_en_la_red_hasta_que_vence() {
    let s = setup();
    authorized(&s);
    let ttl = s.env.as_contract(&s.splitter.address, || {
        s.env
            .storage()
            .persistent()
            .get_ttl(&DataKey::Agent(s.payer.clone()))
    });
    assert_eq!(ttl, (7 * DAY / 5) as u32 + 7 * DAY_LEDGERS);
}

// ---------- pay_batch_as_agent ----------

#[test]
fn el_agente_paga_varios_articulos_con_una_firma_dentro_del_tope_y_del_presupuesto() {
    let s = setup();
    let agent = authorized(&s);
    let other_publisher = Address::generate(&s.env);
    let before = s.usdc.balance(&s.payer);
    let items = vec![
        &s.env,
        item(&s.env, "crispr-nanomedicine-biotech", &s.publisher, PRICE),
        item(
            &s.env,
            "quantum-cryptography-post-quantum",
            &other_publisher,
            3_000_000,
        ),
    ];
    let split = s.splitter.pay_batch_as_agent(&agent, &s.payer, &items);
    let receipts = s.receipts();
    let auths = s.env.auths();

    assert_eq!(
        split,
        Split {
            publisher: 4_900_000 + 2_940_000,
            fee: 100_000 + 60_000
        }
    );
    assert_eq!(s.usdc.balance(&s.payer), before - 8_000_000);
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&other_publisher), 2_940_000);
    assert_eq!(s.usdc.balance(&s.treasury), 160_000);
    assert_eq!(s.usdc.balance(&agent), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(allowance(&s, &s.payer), BUDGET - 8_000_000);
    let second = String::from_str(&s.env, "quantum-cryptography-post-quantum");
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
            AgentPaid {
                owner: s.payer.clone(),
                agent: agent.clone(),
                paper_id: paper(&s.env),
                amount: PRICE,
                remaining: BUDGET - PRICE,
            }
            .to_xdr(&s.env, &s.splitter.address),
            Paid {
                publisher: other_publisher.clone(),
                reader: s.payer.clone(),
                paper_id: second.clone(),
                payer: s.payer.clone(),
                amount: 3_000_000,
                fee: 60_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            AgentPaid {
                owner: s.payer.clone(),
                agent: agent.clone(),
                paper_id: second,
                amount: 3_000_000,
                remaining: BUDGET - 8_000_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );
    assert_eq!(
        auths,
        std::vec![(
            agent.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "pay_batch_as_agent"),
                    (agent.clone(), s.payer.clone(), items.clone()).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
}

#[test]
fn el_paquete_del_agente_es_todo_o_nada_y_respeta_cada_limite() {
    let s = setup();
    let agent = authorized(&s);
    let before = s.usdc.balance(&s.payer);
    let batch = |list: &Vec<Item>| s.splitter.try_pay_batch_as_agent(&agent, &s.payer, list);
    let one = |id: &str, amount: i128| item(&s.env, id, &s.publisher, amount);

    assert_eq!(batch(&Vec::new(&s.env)), Err(Ok(Error::EmptyBatch.into())));
    let mut too_big: Vec<Item> = Vec::new(&s.env);
    for _ in 0..=MAX_BATCH_ITEMS {
        too_big.push_back(one("a", 1));
    }
    assert_eq!(batch(&too_big), Err(Ok(Error::BatchTooLarge.into())));
    assert_eq!(
        batch(&vec![&s.env, one("a", PRICE), one("a", PRICE)]),
        Err(Ok(Error::DuplicatePaper.into()))
    );
    // El segundo pasa del tope por lectura: tampoco se cobra el primero.
    assert_eq!(
        batch(&vec![&s.env, one("a", PRICE), one("b", CAP + 1)]),
        Err(Ok(Error::AboveMaxPerRead.into()))
    );
    // Cinco lecturas al tope pasan del presupuesto de 2.00: no se cobra ninguna.
    assert_eq!(
        batch(&vec![
            &s.env,
            one("a", CAP),
            one("b", CAP),
            one("c", CAP),
            one("d", CAP),
            one("e", CAP)
        ]),
        Err(Ok(Error::BudgetExhausted.into()))
    );
    assert_eq!(
        batch(&vec![&s.env, one("a", PRICE), one("b", 0)]),
        Err(Ok(Error::InvalidAmount.into()))
    );
    let stranger = Address::generate(&s.env);
    assert_eq!(
        s.splitter
            .try_pay_batch_as_agent(&stranger, &s.payer, &vec![&s.env, one("a", PRICE)]),
        Err(Ok(Error::NoAgent.into()))
    );
    s.env.set_auths(&[]);
    assert!(batch(&vec![&s.env, one("a", PRICE)]).is_err());
    s.env.mock_all_auths();
    assert_eq!(s.usdc.balance(&s.payer), before);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(allowance(&s, &s.payer), BUDGET);

    // Cuatro al tope sí: justo el presupuesto.
    assert!(batch(&vec![
        &s.env,
        one("a", CAP),
        one("b", CAP),
        one("c", CAP),
        one("d", CAP)
    ])
    .is_ok());
    assert_eq!(allowance(&s, &s.payer), 0);
    assert_eq!(s.usdc.balance(&s.payer), before - BUDGET);

    s.advance(7 * DAY);
    assert_eq!(
        batch(&vec![&s.env, one("e", 1)]),
        Err(Ok(Error::AgentExpired.into()))
    );
}
