//! Journal club (v0.12.0): `create_club` y `propose_read` (soporte), `join_club`, `approve_read`
//! y `leave_club`.

use super::*;
use crate::{
    Club, ClubCreated, ClubJoined, ClubLeft, ClubPaid, Proposal, ReadApproved, ReadProposed,
    MAX_CLUB_MEMBERS,
};

/// La cuota de prueba: 0.50 USDC.
const DUES: i128 = 5_000_000;

struct Group {
    id: u32,
    ana: Address,
    beto: Address,
    caro: Address,
}

impl Setup<'_> {
    /// Una cuenta con 1 USDC.
    fn person(&self) -> Address {
        let person = Address::generate(&self.env);
        self.fund(&person, 10_000_000);
        person
    }

    /// Un club de tres miembros (Ana, Beto y Caro) con cuota de 0.50 y el quórum dado: 1.50 en caja.
    fn club_of_three(&self, quorum: u32) -> Group {
        let ana = self.person();
        let beto = self.person();
        let caro = self.person();
        let id = self.splitter.create_club(&ana, &DUES, &quorum);
        for member in [&ana, &beto, &caro] {
            self.splitter.join_club(member, &id);
        }
        Group {
            id,
            ana,
            beto,
            caro,
        }
    }

    fn club(&self, id: u32) -> Club {
        self.splitter.club(&id).unwrap()
    }

    /// Ana propone pagar `amount` a `self.publisher` por el artículo de prueba.
    fn proposed(&self, group: &Group, amount: i128) -> u32 {
        self.splitter.propose_read(
            &group.ana,
            &group.id,
            &self.publisher,
            &paper(&self.env),
            &amount,
        )
    }
}

// ---------- create_club (soporte) ----------

#[test]
fn crear_un_club_no_mueve_fondos_y_nadie_es_miembro_hasta_pagar_su_cuota() {
    let s = setup();
    let founder = s.person();
    let id = s.splitter.create_club(&founder, &DUES, &2);
    let receipts = s.receipts();

    assert_eq!(id, 0);
    assert_eq!(
        s.splitter.club(&id),
        Some(Club {
            dues: DUES,
            quorum: 2,
            members: Vec::new(&s.env),
            balance: 0,
            proposals: 0,
        })
    );
    assert_eq!(s.usdc.balance(&founder), 10_000_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    let expected = ClubCreated {
        founder: founder.clone(),
        club_id: 0,
        dues: DUES,
        quorum: 2,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    assert_eq!(s.splitter.create_club(&founder, &1, &MAX_CLUB_MEMBERS), 1);
    assert_eq!(s.splitter.club(&2), None);
}

#[test]
fn crear_un_club_pide_la_firma_de_quien_lo_crea_y_rechaza_cuota_y_quorum_invalidos() {
    let s = setup();
    let founder = s.person();
    s.splitter.create_club(&founder, &DUES, &2);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            founder.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "create_club"),
                    (founder.clone(), DUES, 2u32).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );

    for dues in [0, -1] {
        assert_eq!(
            s.splitter.try_create_club(&founder, &dues, &2),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    // Con quórum de 1 no habría club: pagaría quien propone, sin que nadie más apruebe.
    for quorum in [0, 1, MAX_CLUB_MEMBERS + 1] {
        assert_eq!(
            s.splitter.try_create_club(&founder, &DUES, &quorum),
            Err(Ok(Error::InvalidQuorum.into()))
        );
    }
    s.env.set_auths(&[]);
    assert!(s.splitter.try_create_club(&founder, &DUES, &2).is_err());
    assert_eq!(s.splitter.club(&1), None);
}

// ---------- join_club ----------

#[test]
fn cada_miembro_paga_su_cuota_a_la_caja_del_club() {
    let s = setup();
    let ana = s.person();
    let beto = s.person();
    let id = s.splitter.create_club(&ana, &DUES, &2);

    assert_eq!(s.splitter.join_club(&ana, &id), DUES);
    let balance = s.splitter.join_club(&beto, &id);
    let receipts = s.receipts();
    assert_eq!(balance, 2 * DUES);
    assert_eq!(s.usdc.balance(&ana), 10_000_000 - DUES);
    assert_eq!(s.usdc.balance(&beto), 10_000_000 - DUES);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        2 * DUES,
        "las cuotas quedan en el contrato"
    );
    let club = s.club(id);
    assert_eq!(club.members, vec![&s.env, ana.clone(), beto.clone()]);
    assert_eq!(club.balance, 2 * DUES);
    let expected = ClubJoined {
        member: beto.clone(),
        club_id: id,
        dues: DUES,
        balance: 2 * DUES,
        members: 2,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn entrar_solo_pide_la_firma_de_quien_entra_y_no_se_entra_dos_veces_ni_a_un_club_lleno() {
    let s = setup();
    let ana = s.person();
    let id = s.splitter.create_club(&ana, &DUES, &2);
    s.splitter.join_club(&ana, &id);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            ana.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "join_club"),
                    (ana.clone(), id).into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(&ana, &s.splitter.address, DUES)],
            }
        )]
    );

    assert_eq!(
        s.splitter.try_join_club(&ana, &id),
        Err(Ok(Error::AlreadyMember.into()))
    );
    assert_eq!(
        s.splitter.try_join_club(&ana, &99),
        Err(Ok(Error::NoClub.into()))
    );
    let outsider = s.person();
    s.env.set_auths(&[]);
    assert!(s.splitter.try_join_club(&outsider, &id).is_err());
    s.env.mock_all_auths();

    // Diez miembros como máximo.
    for _ in 1..MAX_CLUB_MEMBERS {
        s.splitter.join_club(&s.person(), &id);
    }
    assert_eq!(s.club(id).members.len(), MAX_CLUB_MEMBERS);
    assert_eq!(
        s.splitter.try_join_club(&outsider, &id),
        Err(Ok(Error::ClubFull.into()))
    );
    assert_eq!(s.usdc.balance(&outsider), 10_000_000);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        MAX_CLUB_MEMBERS as i128 * DUES
    );
}

// ---------- propose_read (soporte) ----------

#[test]
fn un_miembro_propone_una_lectura_sin_mover_fondos_y_su_aprobacion_queda_contada() {
    let s = setup();
    let group = s.club_of_three(2);
    let id = s.proposed(&group, PRICE);
    let receipts = s.receipts();

    assert_eq!(id, 0);
    assert_eq!(
        s.splitter.proposal(&group.id, &id),
        Some(Proposal {
            publisher: s.publisher.clone(),
            paper_id: paper(&s.env),
            amount: PRICE,
            approvals: vec![&s.env, group.ana.clone()],
            paid: false,
        })
    );
    let expected = ReadProposed {
        member: group.ana.clone(),
        club_id: group.id,
        proposal_id: 0,
        publisher: s.publisher.clone(),
        paper_id: paper(&s.env),
        amount: PRICE,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        3 * DUES,
        "nada se movió"
    );
    assert_eq!(s.usdc.balance(&s.publisher), 0);

    assert_eq!(s.proposed(&group, PRICE), 1);
    assert_eq!(s.club(group.id).proposals, 2);
    assert_eq!(s.splitter.proposal(&group.id, &2), None);
}

#[test]
fn solo_un_miembro_propone_y_solo_lo_que_alcanza_en_la_caja() {
    let s = setup();
    let group = s.club_of_three(2);
    let stranger = s.person();
    let propose = |member: &Address, club_id: u32, paper_id: &String, amount: i128| {
        s.splitter
            .try_propose_read(member, &club_id, &s.publisher, paper_id, &amount)
    };

    s.proposed(&group, PRICE);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            group.ana.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "propose_read"),
                    (
                        group.ana.clone(),
                        group.id,
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

    assert_eq!(
        propose(&stranger, group.id, &paper(&s.env), PRICE),
        Err(Ok(Error::NotMember.into()))
    );
    assert_eq!(
        propose(&group.ana, 99, &paper(&s.env), PRICE),
        Err(Ok(Error::NoClub.into()))
    );
    for amount in [0, -1] {
        assert_eq!(
            propose(&group.ana, group.id, &paper(&s.env), amount),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    assert_eq!(
        propose(&group.ana, group.id, &String::from_str(&s.env, ""), PRICE),
        Err(Ok(Error::InvalidPaperId.into()))
    );
    // Justo la caja se puede proponer; un stroop más, no.
    assert_eq!(
        propose(&group.ana, group.id, &paper(&s.env), 3 * DUES + 1),
        Err(Ok(Error::ClubTooLow.into()))
    );
    assert_eq!(
        propose(&group.ana, group.id, &paper(&s.env), 3 * DUES),
        Ok(Ok(1))
    );
    s.env.set_auths(&[]);
    assert!(propose(&group.ana, group.id, &paper(&s.env), PRICE).is_err());
    assert_eq!(s.club(group.id).proposals, 2);
}

// ---------- approve_read ----------

#[test]
fn la_aprobacion_que_completa_el_quorum_paga_la_lectura_desde_la_caja() {
    let s = setup();
    let group = s.club_of_three(2);
    let id = s.proposed(&group, PRICE);

    let paid = s.splitter.approve_read(&group.beto, &group.id, &id);
    let receipts = s.receipts();
    assert!(paid);
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 3 * DUES - PRICE);
    assert_eq!(s.club(group.id).balance, 3 * DUES - PRICE);
    for member in [&group.ana, &group.beto, &group.caro] {
        assert_eq!(
            s.usdc.balance(member),
            10_000_000 - DUES,
            "nadie paga de su wallet"
        );
    }
    let proposal = s.splitter.proposal(&group.id, &id).unwrap();
    assert!(proposal.paid);
    assert_eq!(
        proposal.approvals,
        vec![&s.env, group.ana.clone(), group.beto.clone()]
    );
    assert_eq!(
        receipts,
        [
            ClubPaid {
                publisher: s.publisher.clone(),
                club_id: group.id,
                proposal_id: id,
                paper_id: paper(&s.env),
                amount: PRICE,
                fee: 100_000,
                members: vec![
                    &s.env,
                    group.ana.clone(),
                    group.beto.clone(),
                    group.caro.clone()
                ],
            }
            .to_xdr(&s.env, &s.splitter.address),
            ReadApproved {
                member: group.beto.clone(),
                club_id: group.id,
                proposal_id: id,
                approvals: 2,
                paid: true,
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );
}

#[test]
fn antes_del_quorum_una_aprobacion_se_anota_y_no_paga() {
    let s = setup();
    let group = s.club_of_three(3);
    let id = s.proposed(&group, PRICE);

    let paid = s.splitter.approve_read(&group.beto, &group.id, &id);
    let receipts = s.receipts();
    assert!(!paid);
    let expected = ReadApproved {
        member: group.beto.clone(),
        club_id: group.id,
        proposal_id: id,
        approvals: 2,
        paid: false,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert_eq!(s.club(group.id).balance, 3 * DUES);
    assert!(!s.splitter.proposal(&group.id, &id).unwrap().paid);

    // La tercera completa el quórum de 3 y paga.
    assert!(s.splitter.approve_read(&group.caro, &group.id, &id));
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
}

#[test]
fn aprobar_solo_pide_la_firma_del_miembro_aunque_esa_llamada_pague() {
    let s = setup();
    let group = s.club_of_three(2);
    let id = s.proposed(&group, PRICE);
    s.splitter.approve_read(&group.beto, &group.id, &id);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            group.beto.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "approve_read"),
                    (group.beto.clone(), group.id, id).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
}

#[test]
fn cada_miembro_aprueba_una_vez_y_una_propuesta_se_paga_una_vez() {
    let s = setup();
    let group = s.club_of_three(3);
    let stranger = s.person();
    let id = s.proposed(&group, PRICE);
    let approve = |member: &Address, club_id: u32, proposal_id: u32| {
        s.splitter.try_approve_read(member, &club_id, &proposal_id)
    };

    assert_eq!(
        approve(&stranger, group.id, id),
        Err(Ok(Error::NotMember.into()))
    );
    assert_eq!(approve(&group.beto, 99, id), Err(Ok(Error::NoClub.into())));
    assert_eq!(
        approve(&group.beto, group.id, 99),
        Err(Ok(Error::NoProposal.into()))
    );
    // Quien propuso ya aprobó; y nadie aprueba dos veces para completar el quórum solo.
    assert_eq!(
        approve(&group.ana, group.id, id),
        Err(Ok(Error::AlreadyApproved.into()))
    );
    assert_eq!(approve(&group.beto, group.id, id), Ok(Ok(false)));
    assert_eq!(
        approve(&group.beto, group.id, id),
        Err(Ok(Error::AlreadyApproved.into()))
    );
    s.env.set_auths(&[]);
    assert!(approve(&group.caro, group.id, id).is_err());
    assert_eq!(s.usdc.balance(&s.publisher), 0);

    s.env.mock_all_auths();
    assert_eq!(approve(&group.caro, group.id, id), Ok(Ok(true)));
    // Pagada, ya no recibe aprobaciones: un cuarto miembro no la vuelve a pagar.
    let dana = s.person();
    s.splitter.join_club(&dana, &group.id);
    assert_eq!(
        approve(&dana, group.id, id),
        Err(Ok(Error::ProposalClosed.into()))
    );
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.club(group.id).balance, 4 * DUES - PRICE);
}

#[test]
fn la_aprobacion_de_quien_ya_salio_del_club_no_cuenta() {
    let s = setup();
    let group = s.club_of_three(2);
    let id = s.proposed(&group, PRICE);
    // Ana propuso (su aprobación contaba) y sale.
    s.splitter.leave_club(&group.ana, &group.id);

    assert!(!s.splitter.approve_read(&group.beto, &group.id, &id));
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert!(s.splitter.approve_read(&group.caro, &group.id, &id));
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
}

#[test]
fn si_a_la_caja_ya_no_le_alcanza_la_aprobacion_falla_y_no_queda_anotada() {
    let s = setup();
    let group = s.club_of_three(2);
    let first = s.proposed(&group, 10_000_000);
    let second = s.proposed(&group, 10_000_000);
    assert!(s.splitter.approve_read(&group.beto, &group.id, &first));

    // Quedan 0.50 y la segunda propuesta pide 1.00.
    assert_eq!(
        s.splitter.try_approve_read(&group.beto, &group.id, &second),
        Err(Ok(Error::ClubTooLow.into()))
    );
    let proposal = s.splitter.proposal(&group.id, &second).unwrap();
    assert_eq!(proposal.approvals, vec![&s.env, group.ana.clone()]);
    assert!(!proposal.paid);
    assert_eq!(s.club(group.id).balance, 3 * DUES - 10_000_000);
    assert_eq!(s.usdc.balance(&s.publisher), 9_800_000);
}

#[test]
fn si_la_editorial_no_puede_recibir_la_aprobacion_falla_y_la_caja_queda_intacta() {
    let s = setup();
    let group = s.club_of_three(2);
    let nueva = g_account(&s.env, 71);
    let id = s
        .splitter
        .propose_read(&group.ana, &group.id, &nueva, &paper(&s.env), &PRICE);

    assert!(s
        .splitter
        .try_approve_read(&group.beto, &group.id, &id)
        .is_err());
    assert_eq!(s.club(group.id).balance, 3 * DUES);
    assert_eq!(s.usdc.balance(&s.splitter.address), 3 * DUES);
    assert_eq!(s.splitter.owed(&nueva), 0);
    assert!(!s.splitter.proposal(&group.id, &id).unwrap().paid);
}

// ---------- leave_club ----------

#[test]
fn quien_sale_se_lleva_una_parte_igual_de_lo_que_queda_en_la_caja() {
    let s = setup();
    let group = s.club_of_three(2);
    let id = s.proposed(&group, PRICE);
    s.splitter.approve_read(&group.beto, &group.id, &id);
    // Quedan 1.00 en caja entre tres.

    let share = s.splitter.leave_club(&group.caro, &group.id);
    let receipts = s.receipts();
    assert_eq!(share, 3_333_333);
    assert_eq!(s.usdc.balance(&group.caro), 10_000_000 - DUES + 3_333_333);
    let club = s.club(group.id);
    assert_eq!(club.balance, 6_666_667);
    assert_eq!(
        club.members,
        vec![&s.env, group.ana.clone(), group.beto.clone()]
    );
    let expected = ClubLeft {
        member: group.caro.clone(),
        club_id: group.id,
        amount: 3_333_333,
        balance: 6_666_667,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    // Los que quedan parten lo que queda; el último se lleva hasta el stroop del redondeo.
    assert_eq!(s.splitter.leave_club(&group.beto, &group.id), 3_333_333);
    assert_eq!(s.splitter.leave_club(&group.ana, &group.id), 3_333_334);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    let club = s.club(group.id);
    assert_eq!(club.balance, 0);
    assert_eq!(club.members.len(), 0);
}

#[test]
fn salir_solo_pide_la_firma_de_quien_sale_y_fuera_del_club_ya_no_propone_ni_aprueba() {
    let s = setup();
    let group = s.club_of_three(2);
    let stranger = s.person();
    let id = s.proposed(&group, PRICE);

    assert_eq!(
        s.splitter.try_leave_club(&stranger, &group.id),
        Err(Ok(Error::NotMember.into()))
    );
    assert_eq!(
        s.splitter.try_leave_club(&group.caro, &99),
        Err(Ok(Error::NoClub.into()))
    );
    s.env.set_auths(&[]);
    assert!(s.splitter.try_leave_club(&group.caro, &group.id).is_err());
    assert_eq!(s.club(group.id).members.len(), 3);

    s.env.mock_all_auths();
    s.splitter.leave_club(&group.caro, &group.id);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            group.caro.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "leave_club"),
                    (group.caro.clone(), group.id).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
    assert_eq!(
        s.splitter.try_approve_read(&group.caro, &group.id, &id),
        Err(Ok(Error::NotMember.into()))
    );
    assert_eq!(
        s.splitter
            .try_propose_read(&group.caro, &group.id, &s.publisher, &paper(&s.env), &PRICE),
        Err(Ok(Error::NotMember.into()))
    );
    assert_eq!(
        s.splitter.try_leave_club(&group.caro, &group.id),
        Err(Ok(Error::NotMember.into()))
    );
    // Puede volver a entrar pagando la cuota otra vez.
    s.splitter.join_club(&group.caro, &group.id);
    assert_eq!(s.club(group.id).members.len(), 3);
}

// ---------- TTL y custodia ----------

#[test]
fn el_club_y_sus_propuestas_viven_el_ttl_maximo() {
    let s = setup();
    let group = s.club_of_three(3);
    let ttl = |key: DataKey| {
        s.env.as_contract(&s.splitter.address, || {
            s.env.storage().persistent().get_ttl(&key)
        })
    };
    assert_eq!(ttl(DataKey::Club(group.id)), 3_110_400 - 1);

    s.advance(20 * DAY);
    let id = s.proposed(&group, PRICE);
    assert_eq!(ttl(DataKey::Club(group.id)), 3_110_400 - 1);
    assert_eq!(ttl(DataKey::Proposal(group.id, id)), 3_110_400 - 1);

    s.advance(20 * DAY);
    s.splitter.approve_read(&group.beto, &group.id, &id);
    assert_eq!(ttl(DataKey::Club(group.id)), 3_110_400 - 1);
    assert_eq!(ttl(DataKey::Proposal(group.id, id)), 3_110_400 - 1);

    s.advance(20 * DAY);
    s.splitter.leave_club(&group.caro, &group.id);
    assert_eq!(ttl(DataKey::Club(group.id)), 3_110_400 - 1);
    // Un club recién creado ya vive el máximo, y cada miembro que entra lo renueva.
    let founder = s.person();
    let fresh = s.splitter.create_club(&founder, &DUES, &2);
    assert_eq!(ttl(DataKey::Club(fresh)), 3_110_400 - 1);
    s.advance(20 * DAY);
    s.splitter.join_club(&founder, &fresh);
    assert_eq!(ttl(DataKey::Club(fresh)), 3_110_400 - 1);
}

#[test]
fn el_saldo_del_contrato_es_la_suma_de_las_cajas_de_los_clubes() {
    let s = setup();
    let lab = s.club_of_three(2);
    let seminar = s.club_of_three(3);
    let check = || {
        assert_eq!(
            s.usdc.balance(&s.splitter.address),
            s.club(lab.id).balance + s.club(seminar.id).balance
        );
    };
    check();

    let first = s.proposed(&lab, PRICE);
    s.splitter.approve_read(&lab.beto, &lab.id, &first);
    check();
    let other = s.splitter.propose_read(
        &seminar.ana,
        &seminar.id,
        &s.publisher,
        &paper(&s.env),
        &3_000_000,
    );
    s.splitter.approve_read(&seminar.beto, &seminar.id, &other);
    check();
    s.splitter.leave_club(&lab.caro, &lab.id);
    s.splitter.approve_read(&seminar.caro, &seminar.id, &other);
    check();
    for member in [&lab.ana, &lab.beto] {
        s.splitter.leave_club(member, &lab.id);
    }
    for member in [&seminar.ana, &seminar.beto, &seminar.caro] {
        s.splitter.leave_club(member, &seminar.id);
    }
    check();
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000 + 2_940_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000 + 60_000);
}
