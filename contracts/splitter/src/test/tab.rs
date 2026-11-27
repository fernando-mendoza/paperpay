//! Cuenta abierta (v0.10.0): `open_tab`, `charge_tab` (soporte), `settle_tab`, `collect_tab` y
//! `close_tab`.

use super::*;
use crate::{Charged, Tab, TabClosed, TabCollected, TabOpened, TabSettled, MAX_TAB_PUBLISHERS};
use soroban_sdk::Map;

/// La garantía de prueba: 2 USDC.
const DEPOSIT: i128 = 20_000_000;

struct Account {
    reader: Address,
    operator: Address,
}

impl Setup<'_> {
    /// Una cuenta abierta con 2 USDC de garantía y 7 días para pagar. Quien lee tiene 1 USDC más.
    fn opened_tab(&self) -> Account {
        let reader = Address::generate(&self.env);
        let operator = Address::generate(&self.env);
        self.fund(&reader, DEPOSIT + 10_000_000);
        self.splitter.open_tab(&reader, &operator, &DEPOSIT, &7);
        Account { reader, operator }
    }

    fn charge(&self, account: &Account, publisher: &Address, amount: i128) -> i128 {
        self.splitter.charge_tab(
            &account.operator,
            &account.reader,
            publisher,
            &paper(&self.env),
            &amount,
        )
    }

    fn tab_of(&self, account: &Account) -> Tab {
        self.splitter.tab(&account.reader).unwrap()
    }
}

// ---------- open_tab ----------

#[test]
fn abrir_una_cuenta_deja_la_garantia_en_el_contrato() {
    let s = setup();
    let reader = Address::generate(&s.env);
    let operator = Address::generate(&s.env);
    s.fund(&reader, DEPOSIT + PRICE);
    s.splitter.open_tab(&reader, &operator, &DEPOSIT, &7);
    let receipts = s.receipts();

    assert_eq!(s.usdc.balance(&reader), PRICE);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        DEPOSIT,
        "la garantía queda en el contrato"
    );
    assert_eq!(
        s.splitter.tab(&reader),
        Some(Tab {
            operator: operator.clone(),
            deposit: DEPOSIT,
            days: 7,
            owed: 0,
            due: 0,
            debts: Map::new(&s.env),
        })
    );
    let expected = TabOpened {
        reader: reader.clone(),
        operator,
        deposit: DEPOSIT,
        days: 7,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn abrir_solo_pide_la_firma_de_quien_lee() {
    let s = setup();
    let reader = Address::generate(&s.env);
    let operator = Address::generate(&s.env);
    s.fund(&reader, DEPOSIT);
    s.splitter.open_tab(&reader, &operator, &DEPOSIT, &7);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            reader.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "open_tab"),
                    (reader.clone(), operator.clone(), DEPOSIT, 7u32).into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(&reader, &s.splitter.address, DEPOSIT)],
            }
        )]
    );
}

#[test]
fn abrir_rechaza_garantia_y_plazo_invalidos_una_segunda_cuenta_y_sin_firma_no_abre() {
    let s = setup();
    let reader = Address::generate(&s.env);
    let operator = Address::generate(&s.env);
    s.fund(&reader, 3 * DEPOSIT);

    for deposit in [0, -1] {
        assert_eq!(
            s.splitter.try_open_tab(&reader, &operator, &deposit, &7),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    for days in [0, MAX_PASS_DAYS + 1] {
        assert_eq!(
            s.splitter.try_open_tab(&reader, &operator, &DEPOSIT, &days),
            Err(Ok(Error::InvalidDays.into()))
        );
    }
    assert_eq!(s.splitter.tab(&reader), None);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);

    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_open_tab(&reader, &operator, &DEPOSIT, &7)
        .is_err());
    assert_eq!(s.splitter.tab(&reader), None);

    // Los bordes del plazo valen (1 y 31 días), y una cuenta por lector.
    s.env.mock_all_auths();
    s.splitter
        .open_tab(&reader, &operator, &DEPOSIT, &MAX_PASS_DAYS);
    let other = Address::generate(&s.env);
    s.fund(&other, DEPOSIT);
    s.splitter.open_tab(&other, &operator, &DEPOSIT, &1);
    assert_eq!(
        s.splitter.try_open_tab(&reader, &operator, &DEPOSIT, &7),
        Err(Ok(Error::TabExists.into()))
    );
    assert_eq!(s.usdc.balance(&reader), 2 * DEPOSIT);
}

// ---------- charge_tab ----------

#[test]
fn el_operador_anota_lecturas_como_deuda_sin_mover_fondos() {
    let s = setup();
    let account = s.opened_tab();
    let other_publisher = Address::generate(&s.env);
    let opened_at = s.now();

    let owed = s.charge(&account, &s.publisher, PRICE);
    let receipts = s.receipts();
    assert_eq!(owed, PRICE);
    let due = opened_at + 7 * DAY;
    let expected = Charged {
        reader: account.reader.clone(),
        publisher: s.publisher.clone(),
        paper_id: paper(&s.env),
        amount: PRICE,
        owed: PRICE,
        due,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    // Más lecturas se suman por editorial; el plazo corre desde la primera sin pagar.
    s.advance(DAY);
    assert_eq!(s.charge(&account, &other_publisher, 3_000_000), 8_000_000);
    assert_eq!(s.charge(&account, &s.publisher, PRICE), 13_000_000);
    let tab = s.tab_of(&account);
    assert_eq!(tab.owed, 13_000_000);
    assert_eq!(tab.due, due);
    assert_eq!(tab.debts.get(s.publisher.clone()), Some(2 * PRICE));
    assert_eq!(tab.debts.get(other_publisher.clone()), Some(3_000_000));
    assert_eq!(tab.deposit, DEPOSIT);

    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        DEPOSIT,
        "nada se movió"
    );
    assert_eq!(s.usdc.balance(&account.reader), 10_000_000);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(s.usdc.balance(&other_publisher), 0);
}

#[test]
fn solo_el_operador_de_la_cuenta_anota_y_solo_con_su_firma() {
    let s = setup();
    let account = s.opened_tab();
    let stranger = Address::generate(&s.env);
    s.charge(&account, &s.publisher, PRICE);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            account.operator.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "charge_tab"),
                    (
                        account.operator.clone(),
                        account.reader.clone(),
                        s.publisher.clone(),
                        paper(&s.env),
                        PRICE
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );

    for signer in [&stranger, &account.reader, &s.publisher] {
        assert_eq!(
            s.splitter.try_charge_tab(
                signer,
                &account.reader,
                &s.publisher,
                &paper(&s.env),
                &PRICE
            ),
            Err(Ok(Error::NotOperator.into()))
        );
    }
    assert_eq!(
        s.splitter.try_charge_tab(
            &account.operator,
            &stranger,
            &s.publisher,
            &paper(&s.env),
            &PRICE
        ),
        Err(Ok(Error::NoTab.into()))
    );
    for amount in [0, -1] {
        assert_eq!(
            s.splitter.try_charge_tab(
                &account.operator,
                &account.reader,
                &s.publisher,
                &paper(&s.env),
                &amount
            ),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    assert_eq!(
        s.splitter.try_charge_tab(
            &account.operator,
            &account.reader,
            &s.publisher,
            &String::from_str(&s.env, ""),
            &PRICE
        ),
        Err(Ok(Error::InvalidPaperId.into()))
    );
    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_charge_tab(
            &account.operator,
            &account.reader,
            &s.publisher,
            &paper(&s.env),
            &PRICE
        )
        .is_err());
    assert_eq!(s.tab_of(&account).owed, PRICE);
}

#[test]
fn la_deuda_nunca_pasa_de_la_garantia_ni_de_diez_editoriales() {
    let s = setup();
    let account = s.opened_tab();
    let charge = |publisher: &Address, amount: i128| {
        s.splitter.try_charge_tab(
            &account.operator,
            &account.reader,
            publisher,
            &paper(&s.env),
            &amount,
        )
    };

    // Diez editoriales con 0.10 cada una: la undécima no cabe, pero más deuda con una de las diez sí.
    let mut publishers = std::vec::Vec::new();
    for _ in 0..MAX_TAB_PUBLISHERS {
        let publisher = Address::generate(&s.env);
        assert!(charge(&publisher, 1_000_000).is_ok());
        publishers.push(publisher);
    }
    assert_eq!(
        charge(&Address::generate(&s.env), 1_000_000),
        Err(Ok(Error::TabFull.into()))
    );
    assert_eq!(charge(&publishers[0], 1_000_000), Ok(Ok(11_000_000)));

    // Justo hasta la garantía se puede; un stroop más, no.
    assert_eq!(
        charge(&publishers[1], DEPOSIT - 11_000_000 + 1),
        Err(Ok(Error::TabFull.into()))
    );
    assert_eq!(
        charge(&publishers[1], DEPOSIT - 11_000_000),
        Ok(Ok(DEPOSIT))
    );
    assert_eq!(charge(&publishers[1], 1), Err(Ok(Error::TabFull.into())));
    let tab = s.tab_of(&account);
    assert_eq!(tab.owed, DEPOSIT);
    assert_eq!(tab.debts.len(), MAX_TAB_PUBLISHERS);
}

// ---------- settle_tab ----------

#[test]
fn quien_lee_paga_toda_la_deuda_con_una_firma_y_cada_editorial_cobra_su_98() {
    let s = setup();
    let account = s.opened_tab();
    let other_publisher = Address::generate(&s.env);
    s.charge(&account, &s.publisher, PRICE);
    s.charge(&account, &other_publisher, 3_000_000);
    s.charge(&account, &s.publisher, PRICE);
    // Debe 1.30 y tiene 1.00 en la wallet: consigue 0.50 más.
    s.fund(&account.reader, PRICE);

    let split = s.splitter.settle_tab(&account.reader);
    let receipts = s.receipts();
    assert_eq!(
        split,
        Split {
            publisher: 12_740_000,
            fee: 260_000
        }
    );
    assert_eq!(s.usdc.balance(&s.publisher), 9_800_000);
    assert_eq!(s.usdc.balance(&other_publisher), 2_940_000);
    assert_eq!(s.usdc.balance(&s.treasury), 260_000);
    assert_eq!(
        s.usdc.balance(&account.reader),
        2_000_000,
        "pagó 1.30 de su wallet"
    );
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        DEPOSIT,
        "la garantía no se tocó"
    );
    assert_eq!(
        s.tab_of(&account),
        Tab {
            operator: account.operator.clone(),
            deposit: DEPOSIT,
            days: 7,
            owed: 0,
            due: 0,
            debts: Map::new(&s.env),
        }
    );
    let expected = TabSettled {
        reader: account.reader.clone(),
        total: 13_000_000,
        fee: 260_000,
        publishers: 2,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn pagar_solo_pide_la_firma_de_quien_lee_sobre_el_total() {
    let s = setup();
    let account = s.opened_tab();
    s.charge(&account, &s.publisher, PRICE);
    s.charge(&account, &Address::generate(&s.env), 3_000_000);
    s.splitter.settle_tab(&account.reader);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            account.reader.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "settle_tab"),
                    (account.reader.clone(),).into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(
                    &account.reader,
                    &s.splitter.address,
                    8_000_000
                )],
            }
        )]
    );
}

#[test]
fn sin_deuda_no_hay_nada_que_pagar_y_despues_de_pagar_el_plazo_arranca_de_nuevo() {
    let s = setup();
    let account = s.opened_tab();
    let stranger = Address::generate(&s.env);

    assert_eq!(
        s.splitter.try_settle_tab(&account.reader),
        Err(Ok(Error::NothingOwed.into()))
    );
    assert_eq!(
        s.splitter.try_settle_tab(&stranger),
        Err(Ok(Error::NoTab.into()))
    );
    s.charge(&account, &s.publisher, PRICE);
    s.env.set_auths(&[]);
    assert!(s.splitter.try_settle_tab(&account.reader).is_err());
    assert_eq!(s.usdc.balance(&s.publisher), 0);

    s.env.mock_all_auths();
    s.advance(3 * DAY);
    s.splitter.settle_tab(&account.reader);
    assert_eq!(
        s.splitter.try_settle_tab(&account.reader),
        Err(Ok(Error::NothingOwed.into()))
    );
    // Una lectura nueva arranca un plazo nuevo, desde ahora.
    s.charge(&account, &s.publisher, PRICE);
    assert_eq!(s.tab_of(&account).due, s.now() + 7 * DAY);
}

#[test]
fn si_una_editorial_no_puede_recibir_su_parte_queda_pendiente_y_las_demas_cobran() {
    let s = setup();
    let account = s.opened_tab();
    let nueva = g_account(&s.env, 51);
    s.charge(&account, &nueva, PRICE);
    s.charge(&account, &s.publisher, PRICE);
    s.splitter.settle_tab(&account.reader);

    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.splitter.owed(&nueva), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 200_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), DEPOSIT + 4_900_000);
    assert_eq!(s.tab_of(&account).owed, 0);
}

#[test]
fn las_comisiones_se_redondean_por_editorial_y_no_se_pierde_ni_un_stroop() {
    let s = setup();
    let account = s.opened_tab();
    let a = Address::generate(&s.env);
    let b = Address::generate(&s.env);
    // 2% de 49 es 0.98 → 0; 2% de 151 es 3.02 → 3.
    s.charge(&account, &a, 49);
    s.charge(&account, &b, 151);
    let split = s.splitter.settle_tab(&account.reader);

    assert_eq!(
        split,
        Split {
            publisher: 197,
            fee: 3
        }
    );
    assert_eq!(s.usdc.balance(&a), 49);
    assert_eq!(s.usdc.balance(&b), 148);
    assert_eq!(s.usdc.balance(&s.treasury), 3);
    assert_eq!(s.usdc.balance(&s.splitter.address), DEPOSIT);
}

// ---------- collect_tab ----------

#[test]
fn vencido_el_plazo_cualquiera_cobra_la_deuda_de_la_garantia() {
    let s = setup();
    let account = s.opened_tab();
    let other_publisher = Address::generate(&s.env);
    s.charge(&account, &s.publisher, PRICE);
    s.charge(&account, &other_publisher, 3_000_000);

    // Un segundo antes del plazo la garantía no se toca; en el plazo, sí.
    s.advance(7 * DAY - 5);
    s.env.ledger().with_mut(|li| li.timestamp += 4);
    assert_eq!(
        s.splitter.try_collect_tab(&account.reader),
        Err(Ok(Error::TabNotDue.into()))
    );
    s.env.ledger().with_mut(|li| li.timestamp += 1);
    // Sin ninguna firma.
    s.env.set_auths(&[]);
    let split = s.splitter.collect_tab(&account.reader);
    let receipts = s.receipts();
    assert_eq!(s.env.auths(), std::vec![]);

    assert_eq!(
        split,
        Split {
            publisher: 7_840_000,
            fee: 160_000
        }
    );
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&other_publisher), 2_940_000);
    assert_eq!(s.usdc.balance(&s.treasury), 160_000);
    assert_eq!(
        s.usdc.balance(&account.reader),
        10_000_000,
        "no salió nada de su wallet"
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), DEPOSIT - 8_000_000);
    let tab = s.tab_of(&account);
    assert_eq!(tab.deposit, DEPOSIT - 8_000_000);
    assert_eq!(tab.owed, 0);
    assert_eq!(tab.due, 0);
    assert_eq!(tab.debts.len(), 0);
    let expected = TabCollected {
        reader: account.reader.clone(),
        total: 8_000_000,
        fee: 160_000,
        publishers: 2,
        deposit: DEPOSIT - 8_000_000,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn no_se_cobra_de_la_garantia_sin_deuda_ni_dos_veces_ni_despues_de_pagar() {
    let s = setup();
    let account = s.opened_tab();
    let stranger = Address::generate(&s.env);

    assert_eq!(
        s.splitter.try_collect_tab(&account.reader),
        Err(Ok(Error::NothingOwed.into()))
    );
    assert_eq!(
        s.splitter.try_collect_tab(&stranger),
        Err(Ok(Error::NoTab.into()))
    );

    // Quien paga a tiempo no pierde garantía, aunque el plazo viejo ya haya pasado.
    s.charge(&account, &s.publisher, PRICE);
    s.splitter.settle_tab(&account.reader);
    s.advance(30 * DAY);
    assert_eq!(
        s.splitter.try_collect_tab(&account.reader),
        Err(Ok(Error::NothingOwed.into()))
    );

    s.charge(&account, &s.publisher, PRICE);
    assert_eq!(
        s.splitter.try_collect_tab(&account.reader),
        Err(Ok(Error::TabNotDue.into()))
    );
    s.advance(7 * DAY);
    s.splitter.collect_tab(&account.reader);
    assert_eq!(
        s.splitter.try_collect_tab(&account.reader),
        Err(Ok(Error::NothingOwed.into()))
    );
    assert_eq!(s.tab_of(&account).deposit, DEPOSIT - PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 2 * 4_900_000);
}

// ---------- close_tab ----------

#[test]
fn sin_deuda_quien_lee_cierra_la_cuenta_y_recupera_lo_que_queda_de_garantia() {
    let s = setup();
    let account = s.opened_tab();
    s.charge(&account, &s.publisher, PRICE);
    assert_eq!(
        s.splitter.try_close_tab(&account.reader),
        Err(Ok(Error::TabOwes.into()))
    );
    s.advance(7 * DAY);
    s.splitter.collect_tab(&account.reader);

    let back = s.splitter.close_tab(&account.reader);
    let receipts = s.receipts();
    assert_eq!(back, DEPOSIT - PRICE);
    assert_eq!(
        s.usdc.balance(&account.reader),
        10_000_000 + DEPOSIT - PRICE
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.splitter.tab(&account.reader), None);
    let expected = TabClosed {
        reader: account.reader.clone(),
        amount: DEPOSIT - PRICE,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    // Cerrada, el operador ya no anota nada, y se puede abrir otra.
    assert_eq!(
        s.splitter.try_charge_tab(
            &account.operator,
            &account.reader,
            &s.publisher,
            &paper(&s.env),
            &PRICE
        ),
        Err(Ok(Error::NoTab.into()))
    );
    assert_eq!(
        s.splitter.try_close_tab(&account.reader),
        Err(Ok(Error::NoTab.into()))
    );
    s.splitter
        .open_tab(&account.reader, &account.operator, &PRICE, &1);
    assert_eq!(s.tab_of(&account).deposit, PRICE);
}

#[test]
fn cerrar_solo_pide_la_firma_de_quien_lee_y_el_operador_no_cierra() {
    let s = setup();
    let account = s.opened_tab();

    assert_eq!(
        s.splitter.try_close_tab(&account.operator),
        Err(Ok(Error::NoTab.into()))
    );
    s.env.set_auths(&[]);
    assert!(s.splitter.try_close_tab(&account.reader).is_err());
    assert_eq!(s.usdc.balance(&s.splitter.address), DEPOSIT);

    s.env.mock_all_auths();
    s.splitter.close_tab(&account.reader);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            account.reader.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "close_tab"),
                    (account.reader.clone(),).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
    assert_eq!(s.usdc.balance(&account.reader), DEPOSIT + 10_000_000);
}

// ---------- TTL y custodia ----------

#[test]
fn la_cuenta_abierta_vive_el_ttl_maximo_y_cada_uso_la_renueva() {
    let s = setup();
    let account = s.opened_tab();
    let key = DataKey::Tab(account.reader.clone());
    let ttl = || {
        s.env.as_contract(&s.splitter.address, || {
            s.env.storage().persistent().get_ttl(&key)
        })
    };
    assert_eq!(ttl(), 3_110_400 - 1);
    s.advance(20 * DAY);
    s.charge(&account, &s.publisher, PRICE);
    assert_eq!(ttl(), 3_110_400 - 1, "anotar una lectura la renueva");
    s.advance(20 * DAY);
    s.splitter.settle_tab(&account.reader);
    assert_eq!(ttl(), 3_110_400 - 1, "pagar también");
    s.charge(&account, &s.publisher, PRICE);
    s.advance(20 * DAY);
    s.splitter.collect_tab(&account.reader);
    assert_eq!(ttl(), 3_110_400 - 1, "y cobrar de la garantía");
}

#[test]
fn el_saldo_del_contrato_cuenta_las_garantias_y_lo_pendiente() {
    let s = setup();
    let ana = s.opened_tab();
    let beto = s.opened_tab();
    let nueva = g_account(&s.env, 53);
    let check = || {
        let mut held = s.splitter.owed(&nueva);
        for account in [&ana, &beto] {
            if let Some(tab) = s.splitter.tab(&account.reader) {
                held += tab.deposit;
            }
        }
        assert_eq!(s.usdc.balance(&s.splitter.address), held);
    };
    check();

    s.charge(&ana, &s.publisher, PRICE);
    s.charge(&ana, &nueva, 3_000_000);
    s.charge(&beto, &nueva, PRICE);
    check();
    s.splitter.settle_tab(&ana.reader);
    check();
    assert_eq!(s.splitter.owed(&nueva), 2_940_000);

    s.advance(7 * DAY);
    s.splitter.collect_tab(&beto.reader);
    check();
    assert_eq!(s.splitter.owed(&nueva), 2_940_000 + 4_900_000);
    s.splitter.close_tab(&ana.reader);
    check();
    s.splitter.claim(&nueva);
    s.splitter.close_tab(&beto.reader);
    check();
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(
        s.usdc.balance(&ana.reader),
        DEPOSIT + 10_000_000 - 8_000_000
    );
    assert_eq!(s.usdc.balance(&beto.reader), DEPOSIT + 10_000_000 - PRICE);
}
