//! Vales de lectura (v0.9.0): `buy_vouchers`, `redeem_voucher` y `reclaim_vouchers`.

use super::*;
use crate::{Voucher, VoucherRedeemed, VouchersBought, VouchersReclaimed, MAX_VOUCHER_DAYS};
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{xdr::ToXdr, BytesN};

/// El mensaje de un canje, generado fuera de Rust con stellar-cli:
/// `echo '{"vec":[{"address":"CAYV…G6VS"},{"address":"GAB2…CWRV"},{"string":"crispr-nanomedicine-biotech"}]}' | stellar xdr encode --type ScVal`
/// (en hexadecimal). Así el script que firma y el contrato coinciden byte por byte.
const VECTOR_HEX: &str = "000000100000000100000003000000120000000131588435e8a57eabc03df2746d4997f4efbaa791ed7d7d98074edacdfc6f5c7300000012000000000000000003a6dc2fcb1247e5f760004c8d20fc83ecc2279cf4a2f33f5b1c4025527f12110000000e0000001b6372697370722d6e616e6f6d65646963696e652d62696f7465636800";
const VECTOR_CONTRACT: &str = "CAYVRBBV5CSX5K6AHXZHI3KJS72O7OVHSHWX27MYA5HNVTP4N5OHG6VS";
const VECTOR_READER: &str = "GAB2NXBPZMJEPZPXMAAEZDJA7SB6ZQRHTT2KF4Z7LMOEAJKSP4JBCWRV";

type Key = (SigningKey, BytesN<32>);

/// Un par de llaves de un vale: la privada va en el enlace; la pública, al contrato.
fn voucher_key(env: &Env, seed: u8) -> Key {
    let sk = SigningKey::from_bytes(&[seed; 32]);
    let key = BytesN::from_array(env, &sk.verifying_key().to_bytes());
    (sk, key)
}

fn sign_raw(env: &Env, sk: &SigningKey, raw: &[u8]) -> BytesN<64> {
    BytesN::from_array(env, &sk.sign(raw).to_bytes())
}

/// Lo que firma quien tiene un vale: (contrato, lector, artículo) en XDR.
fn sign(
    env: &Env,
    contract: &Address,
    sk: &SigningKey,
    reader: &Address,
    paper_id: &String,
) -> BytesN<64> {
    let message = (contract.clone(), reader.clone(), paper_id.clone()).to_xdr(env);
    let raw: std::vec::Vec<u8> = message.iter().collect();
    sign_raw(env, sk, &raw)
}

fn hex(text: &str) -> std::vec::Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

struct Bought {
    buyer: Address,
    keys: std::vec::Vec<Key>,
    expires_at: u64,
}

impl Setup<'_> {
    /// Un vale de 0.50 por cada semilla, para el catálogo de `self.publisher`, vigentes 7 días.
    fn bought_vouchers(&self, seeds: &[u8]) -> Bought {
        let buyer = Address::generate(&self.env);
        self.fund(&buyer, PRICE * seeds.len() as i128);
        let keys: std::vec::Vec<Key> = seeds.iter().map(|s| voucher_key(&self.env, *s)).collect();
        let expires_at = self.now() + 7 * DAY;
        self.splitter.buy_vouchers(
            &buyer,
            &self.publisher,
            &self.public_keys(&keys),
            &PRICE,
            &expires_at,
        );
        Bought {
            buyer,
            keys,
            expires_at,
        }
    }

    fn public_keys(&self, keys: &[Key]) -> Vec<BytesN<32>> {
        let mut list = Vec::new(&self.env);
        for (_, key) in keys {
            list.push_back(key.clone());
        }
        list
    }

    /// Firma y canjea el vale `key` para `reader` y `paper_id`.
    fn redeem(
        &self,
        key: &Key,
        reader: &Address,
        paper_id: &String,
    ) -> Result<Split, Result<soroban_sdk::Error, soroban_sdk::InvokeError>> {
        let signature = sign(&self.env, &self.splitter.address, &key.0, reader, paper_id);
        match self
            .splitter
            .try_redeem_voucher(&key.1, &signature, reader, paper_id)
        {
            Ok(split) => Ok(split.unwrap()),
            Err(err) => Err(err),
        }
    }
}

// ---------- buy_vouchers ----------

#[test]
fn comprar_vales_deja_el_total_en_garantia_y_guarda_solo_las_llaves_publicas() {
    let s = setup();
    let buyer = Address::generate(&s.env);
    s.fund(&buyer, 4 * PRICE);
    let keys = [
        voucher_key(&s.env, 1),
        voucher_key(&s.env, 2),
        voucher_key(&s.env, 3),
    ];
    let public = s.public_keys(&keys);
    let expires_at = s.now() + 7 * DAY;
    let total = s
        .splitter
        .buy_vouchers(&buyer, &s.publisher, &public, &PRICE, &expires_at);
    let receipts = s.receipts();

    assert_eq!(total, 3 * PRICE);
    assert_eq!(s.usdc.balance(&buyer), PRICE);
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        3 * PRICE,
        "el total queda en el contrato"
    );
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    for (_, key) in &keys {
        assert_eq!(
            s.splitter.voucher(key),
            Some(Voucher {
                buyer: buyer.clone(),
                publisher: s.publisher.clone(),
                value: PRICE,
                expires_at,
                open: true,
            })
        );
    }
    assert_eq!(s.splitter.voucher(&voucher_key(&s.env, 9).1), None);
    let expected = VouchersBought {
        buyer: buyer.clone(),
        publisher: s.publisher.clone(),
        keys: public,
        value: PRICE,
        expires_at,
        total: 3 * PRICE,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);
}

#[test]
fn comprar_solo_pide_la_firma_de_quien_compra() {
    let s = setup();
    let buyer = Address::generate(&s.env);
    s.fund(&buyer, 2 * PRICE);
    let public = s.public_keys(&[voucher_key(&s.env, 1), voucher_key(&s.env, 2)]);
    let expires_at = s.now() + DAY;
    s.splitter
        .buy_vouchers(&buyer, &s.publisher, &public, &PRICE, &expires_at);

    assert_eq!(
        s.env.auths(),
        std::vec![(
            buyer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "buy_vouchers"),
                    (
                        buyer.clone(),
                        s.publisher.clone(),
                        public.clone(),
                        PRICE,
                        expires_at
                    )
                        .into_val(&s.env),
                )),
                sub_invocations: std::vec![s.transfer_auth(&buyer, &s.splitter.address, 2 * PRICE)],
            }
        )]
    );
}

#[test]
fn comprar_rechaza_listas_montos_y_vencimientos_invalidos_y_sin_firma_no_hay_vales() {
    let s = setup();
    let buyer = Address::generate(&s.env);
    s.fund(&buyer, 20 * PRICE);
    let now = s.now();
    let max = MAX_VOUCHER_DAYS as u64 * DAY;
    let one = s.public_keys(&[voucher_key(&s.env, 1)]);
    let buy = |keys: &Vec<BytesN<32>>, value: i128, expires_at: u64| {
        s.splitter
            .try_buy_vouchers(&buyer, &s.publisher, keys, &value, &expires_at)
    };

    assert_eq!(
        buy(&Vec::new(&s.env), PRICE, now + DAY),
        Err(Ok(Error::EmptyBatch.into()))
    );
    let mut many = Vec::new(&s.env);
    for seed in 0..=MAX_BATCH_ITEMS {
        many.push_back(voucher_key(&s.env, 100 + seed as u8).1);
    }
    assert_eq!(
        buy(&many, PRICE, now + DAY),
        Err(Ok(Error::BatchTooLarge.into()))
    );
    for value in [0, -1] {
        assert_eq!(
            buy(&one, value, now + DAY),
            Err(Ok(Error::InvalidAmount.into()))
        );
    }
    for expires_at in [now, now - 1, now + max + 1] {
        assert_eq!(
            buy(&one, PRICE, expires_at),
            Err(Ok(Error::InvalidDeadline.into()))
        );
    }
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(s.splitter.voucher(&one.get_unchecked(0)), None);

    // Los bordes valen: diez llaves, un segundo de vigencia y justo el máximo.
    many.pop_back();
    assert_eq!(buy(&many, PRICE, now + 1), Ok(Ok(10 * PRICE)));
    assert!(buy(&one, PRICE, now + max).is_ok());

    s.env.set_auths(&[]);
    let other = s.public_keys(&[voucher_key(&s.env, 2)]);
    assert!(buy(&other, PRICE, now + DAY).is_err());
    assert_eq!(s.usdc.balance(&buyer), 9 * PRICE);
    assert_eq!(s.splitter.voucher(&other.get_unchecked(0)), None);
}

#[test]
fn una_llave_no_se_usa_dos_veces_ni_en_la_misma_compra_ni_despues_de_canjeada() {
    let s = setup();
    let bought = s.bought_vouchers(&[1, 2]);
    let buyer = Address::generate(&s.env);
    s.fund(&buyer, 4 * PRICE);
    let fresh = voucher_key(&s.env, 3);
    let buy = |keys: &[Key]| {
        s.splitter.try_buy_vouchers(
            &buyer,
            &s.publisher,
            &s.public_keys(keys),
            &PRICE,
            &(s.now() + DAY),
        )
    };

    assert_eq!(
        buy(&[fresh.clone(), fresh.clone()]),
        Err(Ok(Error::VoucherExists.into()))
    );
    assert_eq!(
        buy(&[fresh.clone(), bought.keys[0].clone()]),
        Err(Ok(Error::VoucherExists.into()))
    );
    assert_eq!(s.splitter.voucher(&fresh.1), None, "todo o nada");

    // Canjeada o devuelta, la llave sigue ocupada: una firma vieja no sirve para un vale nuevo.
    let reader = Address::generate(&s.env);
    s.redeem(&bought.keys[0], &reader, &paper(&s.env)).unwrap();
    assert_eq!(
        buy(&[bought.keys[0].clone()]),
        Err(Ok(Error::VoucherExists.into()))
    );
    s.advance(7 * DAY);
    s.splitter
        .reclaim_vouchers(&bought.buyer, &s.public_keys(&bought.keys[1..]));
    assert_eq!(
        buy(&[bought.keys[1].clone()]),
        Err(Ok(Error::VoucherExists.into()))
    );
    assert_eq!(s.usdc.balance(&buyer), 4 * PRICE);
}

// ---------- redeem_voucher ----------

#[test]
fn el_canje_paga_la_lectura_con_el_vale_y_no_pide_la_firma_de_ninguna_cuenta() {
    let s = setup();
    let bought = s.bought_vouchers(&[1, 2]);
    let reader = Address::generate(&s.env);
    let key = &bought.keys[0];
    let signature = sign(&s.env, &s.splitter.address, &key.0, &reader, &paper(&s.env));
    // Sin ninguna firma de cuenta: la autorización es la firma del vale.
    s.env.set_auths(&[]);
    let split = s
        .splitter
        .redeem_voucher(&key.1, &signature, &reader, &paper(&s.env));
    let receipts = s.receipts();
    assert_eq!(s.env.auths(), std::vec![]);

    assert_eq!(
        split,
        Split {
            publisher: 4_900_000,
            fee: 100_000
        }
    );
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 100_000);
    assert_eq!(s.usdc.balance(&reader), 0);
    assert_eq!(s.usdc.balance(&bought.buyer), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), PRICE);
    assert!(!s.splitter.voucher(&key.1).unwrap().open);
    assert!(s.splitter.voucher(&bought.keys[1].1).unwrap().open);
    assert_eq!(
        receipts,
        [
            Paid {
                publisher: s.publisher.clone(),
                reader: reader.clone(),
                paper_id: paper(&s.env),
                payer: bought.buyer.clone(),
                amount: PRICE,
                fee: 100_000,
            }
            .to_xdr(&s.env, &s.splitter.address),
            VoucherRedeemed {
                buyer: bought.buyer.clone(),
                reader: reader.clone(),
                key: key.1.clone(),
                paper_id: paper(&s.env),
                value: PRICE,
            }
            .to_xdr(&s.env, &s.splitter.address),
        ]
    );
}

#[test]
fn la_firma_de_un_vale_solo_sirve_para_ese_lector_ese_articulo_y_esa_llave() {
    let s = setup();
    let bought = s.bought_vouchers(&[1, 2]);
    let reader = Address::generate(&s.env);
    let thief = Address::generate(&s.env);
    let other_paper = String::from_str(&s.env, "quantum-cryptography-post-quantum");
    let key = &bought.keys[0];
    let signature = sign(&s.env, &s.splitter.address, &key.0, &reader, &paper(&s.env));

    // Otra cuenta no puede usar esa firma para sí misma ni cambiar el artículo.
    assert!(s
        .splitter
        .try_redeem_voucher(&key.1, &signature, &thief, &paper(&s.env))
        .is_err());
    assert!(s
        .splitter
        .try_redeem_voucher(&key.1, &signature, &reader, &other_paper)
        .is_err());
    // Ni sirve para otro vale, ni una firma hecha con otra llave abre este.
    assert!(s
        .splitter
        .try_redeem_voucher(&bought.keys[1].1, &signature, &reader, &paper(&s.env))
        .is_err());
    let forged = sign(
        &s.env,
        &s.splitter.address,
        &voucher_key(&s.env, 77).0,
        &reader,
        &paper(&s.env),
    );
    assert!(s
        .splitter
        .try_redeem_voucher(&key.1, &forged, &reader, &paper(&s.env))
        .is_err());
    assert_eq!(s.usdc.balance(&s.splitter.address), 2 * PRICE);
    assert_eq!(s.usdc.balance(&s.publisher), 0);
    assert!(s.splitter.voucher(&key.1).unwrap().open);

    // La firma correcta sí.
    assert!(s
        .splitter
        .try_redeem_voucher(&key.1, &signature, &reader, &paper(&s.env))
        .is_ok());
}

#[test]
fn la_firma_de_un_vale_no_sirve_en_otro_contrato() {
    let s = setup();
    let other_id = s.env.register(
        Splitter,
        (
            s.usdc.address.clone(),
            s.treasury.clone(),
            FEE_BPS,
            PASS_DAY_PRICE,
            OPEN_DAY_PRICE,
        ),
    );
    let other = SplitterClient::new(&s.env, &other_id);
    let buyer = Address::generate(&s.env);
    s.fund(&buyer, 2 * PRICE);
    let key = voucher_key(&s.env, 1);
    let public = s.public_keys(std::slice::from_ref(&key));
    let expires_at = s.now() + DAY;
    // La misma llave comprada en los dos contratos.
    s.splitter
        .buy_vouchers(&buyer, &s.publisher, &public, &PRICE, &expires_at);
    other.buy_vouchers(&buyer, &s.publisher, &public, &PRICE, &expires_at);

    let reader = Address::generate(&s.env);
    let signature = sign(&s.env, &s.splitter.address, &key.0, &reader, &paper(&s.env));
    assert!(other
        .try_redeem_voucher(&key.1, &signature, &reader, &paper(&s.env))
        .is_err());
    assert!(s
        .splitter
        .try_redeem_voucher(&key.1, &signature, &reader, &paper(&s.env))
        .is_ok());
    assert_eq!(s.usdc.balance(&other_id), PRICE);
}

#[test]
fn el_mensaje_que_se_firma_es_el_xdr_de_contrato_lector_y_articulo() {
    let s = setup();
    // Un contrato en una dirección fija, para que el mensaje coincida con el vector.
    let contract = Address::from_str(&s.env, VECTOR_CONTRACT);
    s.env.register_at(
        &contract,
        Splitter,
        (
            s.usdc.address.clone(),
            s.treasury.clone(),
            FEE_BPS,
            PASS_DAY_PRICE,
            OPEN_DAY_PRICE,
        ),
    );
    let client = SplitterClient::new(&s.env, &contract);
    let reader = Address::from_str(&s.env, VECTOR_READER);
    let key = voucher_key(&s.env, 1);
    let buyer = Address::generate(&s.env);
    s.fund(&buyer, PRICE);
    client.buy_vouchers(
        &buyer,
        &s.publisher,
        &s.public_keys(std::slice::from_ref(&key)),
        &PRICE,
        &(s.now() + DAY),
    );

    // Lo que arma Rust es el vector, byte por byte…
    let message = (contract.clone(), reader.clone(), paper(&s.env)).to_xdr(&s.env);
    let raw: std::vec::Vec<u8> = message.iter().collect();
    assert_eq!(raw, hex(VECTOR_HEX));
    // …y el contrato acepta una firma hecha sobre los bytes del vector.
    let signature = sign_raw(&s.env, &key.0, &hex(VECTOR_HEX));
    client.redeem_voucher(&key.1, &signature, &reader, &paper(&s.env));
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
}

#[test]
fn un_vale_se_canjea_una_sola_vez_y_una_llave_desconocida_no_canjea_nada() {
    let s = setup();
    let bought = s.bought_vouchers(&[1]);
    let reader = Address::generate(&s.env);
    let unknown = voucher_key(&s.env, 50);

    assert_eq!(
        s.redeem(&unknown, &reader, &paper(&s.env)),
        Err(Ok(Error::NoVoucher.into()))
    );
    assert_eq!(
        s.redeem(&bought.keys[0], &reader, &String::from_str(&s.env, "")),
        Err(Ok(Error::InvalidPaperId.into()))
    );
    assert!(s.redeem(&bought.keys[0], &reader, &paper(&s.env)).is_ok());
    // Ni con la misma firma ni con una nueva para otro lector.
    assert_eq!(
        s.redeem(&bought.keys[0], &reader, &paper(&s.env)),
        Err(Ok(Error::VoucherSpent.into()))
    );
    assert_eq!(
        s.redeem(&bought.keys[0], &Address::generate(&s.env), &paper(&s.env)),
        Err(Ok(Error::VoucherSpent.into()))
    );
    assert_eq!(s.usdc.balance(&s.publisher), 4_900_000);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
}

#[test]
fn un_vale_vencido_no_se_canjea() {
    let s = setup();
    let bought = s.bought_vouchers(&[1, 2]);
    let reader = Address::generate(&s.env);

    // Un segundo antes de vencer todavía se canjea; en el segundo del vencimiento, ya no.
    s.advance(7 * DAY - 5);
    s.env.ledger().with_mut(|li| li.timestamp += 4);
    assert_eq!(s.now(), bought.expires_at - 1);
    assert!(s.redeem(&bought.keys[0], &reader, &paper(&s.env)).is_ok());
    s.env.ledger().with_mut(|li| li.timestamp += 1);
    assert_eq!(
        s.redeem(&bought.keys[1], &reader, &paper(&s.env)),
        Err(Ok(Error::VoucherExpired.into()))
    );
    assert_eq!(s.usdc.balance(&s.splitter.address), PRICE);
}

#[test]
fn si_la_editorial_no_puede_recibir_el_canje_falla_y_el_vale_sigue_vigente() {
    let s = setup();
    let nueva = g_account(&s.env, 41);
    let buyer = Address::generate(&s.env);
    s.fund(&buyer, PRICE);
    let key = voucher_key(&s.env, 1);
    let expires_at = s.now() + DAY;
    s.splitter.buy_vouchers(
        &buyer,
        &nueva,
        &s.public_keys(std::slice::from_ref(&key)),
        &PRICE,
        &expires_at,
    );
    let reader = Address::generate(&s.env);

    assert!(s.redeem(&key, &reader, &paper(&s.env)).is_err());
    assert!(s.splitter.voucher(&key.1).unwrap().open);
    assert_eq!(s.usdc.balance(&s.splitter.address), PRICE);
    assert_eq!(s.splitter.owed(&nueva), 0);
    assert_eq!(s.usdc.balance(&s.treasury), 0);

    // Vencido, quien lo compró lo recupera.
    s.advance(DAY);
    assert_eq!(
        s.splitter
            .reclaim_vouchers(&buyer, &s.public_keys(std::slice::from_ref(&key))),
        PRICE
    );
    assert_eq!(s.usdc.balance(&buyer), PRICE);
}

// ---------- reclaim_vouchers ----------

#[test]
fn los_vales_vencidos_sin_usar_vuelven_a_quien_los_compro() {
    let s = setup();
    let bought = s.bought_vouchers(&[1, 2, 3]);
    let reader = Address::generate(&s.env);
    s.redeem(&bought.keys[0], &reader, &paper(&s.env)).unwrap();
    let unused = s.public_keys(&bought.keys[1..]);

    // Un segundo antes de vencer no se devuelven; en el vencimiento, sí.
    s.advance(7 * DAY - 5);
    s.env.ledger().with_mut(|li| li.timestamp += 4);
    assert_eq!(
        s.splitter.try_reclaim_vouchers(&bought.buyer, &unused),
        Err(Ok(Error::VoucherActive.into()))
    );
    s.env.ledger().with_mut(|li| li.timestamp += 1);
    let total = s.splitter.reclaim_vouchers(&bought.buyer, &unused);
    let receipts = s.receipts();

    assert_eq!(total, 2 * PRICE);
    assert_eq!(s.usdc.balance(&bought.buyer), 2 * PRICE);
    assert_eq!(s.usdc.balance(&s.splitter.address), 0);
    assert_eq!(
        s.usdc.balance(&s.treasury),
        100_000,
        "devolver no paga comisión"
    );
    assert!(!s.splitter.voucher(&bought.keys[1].1).unwrap().open);
    let expected = VouchersReclaimed {
        buyer: bought.buyer.clone(),
        keys: unused.clone(),
        total: 2 * PRICE,
    };
    assert_eq!(receipts, [expected.to_xdr(&s.env, &s.splitter.address)]);

    assert_eq!(
        s.splitter.try_reclaim_vouchers(&bought.buyer, &unused),
        Err(Ok(Error::VoucherSpent.into()))
    );
}

#[test]
fn devolver_es_todo_o_nada_y_solo_de_vales_propios_sin_usar() {
    let s = setup();
    let bought = s.bought_vouchers(&[1, 2]);
    let theirs = s.bought_vouchers(&[3]);
    let reader = Address::generate(&s.env);
    s.redeem(&bought.keys[0], &reader, &paper(&s.env)).unwrap();
    s.advance(7 * DAY);
    let reclaim = |keys: &[Key]| {
        s.splitter
            .try_reclaim_vouchers(&bought.buyer, &s.public_keys(keys))
    };

    // Con uno ya canjeado, uno ajeno, uno repetido o uno desconocido, no se devuelve ninguno.
    assert_eq!(
        reclaim(&[bought.keys[1].clone(), bought.keys[0].clone()]),
        Err(Ok(Error::VoucherSpent.into()))
    );
    assert_eq!(
        reclaim(&[bought.keys[1].clone(), theirs.keys[0].clone()]),
        Err(Ok(Error::NoVoucher.into()))
    );
    assert_eq!(
        reclaim(&[bought.keys[1].clone(), bought.keys[1].clone()]),
        Err(Ok(Error::VoucherSpent.into()))
    );
    assert_eq!(
        reclaim(&[bought.keys[1].clone(), voucher_key(&s.env, 60)]),
        Err(Ok(Error::NoVoucher.into()))
    );
    assert_eq!(reclaim(&[]), Err(Ok(Error::EmptyBatch.into())));
    assert!(s.splitter.voucher(&bought.keys[1].1).unwrap().open);
    assert_eq!(s.usdc.balance(&bought.buyer), 0);
    assert_eq!(s.usdc.balance(&s.splitter.address), 2 * PRICE);

    assert_eq!(reclaim(&[bought.keys[1].clone()]), Ok(Ok(PRICE)));
}

#[test]
fn devolver_solo_pide_la_firma_de_quien_compro() {
    let s = setup();
    let bought = s.bought_vouchers(&[1]);
    let keys = s.public_keys(&bought.keys);
    s.advance(7 * DAY);

    s.env.set_auths(&[]);
    assert!(s
        .splitter
        .try_reclaim_vouchers(&bought.buyer, &keys)
        .is_err());
    assert_eq!(s.usdc.balance(&s.splitter.address), PRICE);

    s.env.mock_all_auths();
    s.splitter.reclaim_vouchers(&bought.buyer, &keys);
    assert_eq!(
        s.env.auths(),
        std::vec![(
            bought.buyer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.splitter.address.clone(),
                    Symbol::new(&s.env, "reclaim_vouchers"),
                    (bought.buyer.clone(), keys.clone()).into_val(&s.env),
                )),
                sub_invocations: std::vec![],
            }
        )]
    );
}

// ---------- TTL y custodia ----------

#[test]
fn el_vale_vive_en_la_red_hasta_su_vencimiento_y_la_gracia_para_devolverlo() {
    let s = setup();
    let bought = s.bought_vouchers(&[1]);
    let ttl = s.env.as_contract(&s.splitter.address, || {
        s.env
            .storage()
            .persistent()
            .get_ttl(&DataKey::Voucher(bought.keys[0].1.clone()))
    });
    assert_eq!(
        ttl,
        ((7 + REFUND_GRACE_DAYS) as u64 * DAY / 5) as u32 + 7 * DAY_LEDGERS
    );
}

#[test]
fn el_saldo_del_contrato_cuenta_los_vales_sin_usar() {
    let s = setup();
    let bought = s.bought_vouchers(&[1, 2, 3]);
    let other = s.bought_vouchers(&[4]);
    let sponsor = Address::generate(&s.env);
    let operator = Address::generate(&s.env);
    s.fund(&sponsor, 2 * PRICE);
    let all: std::vec::Vec<Key> = bought
        .keys
        .iter()
        .chain(other.keys.iter())
        .cloned()
        .collect();
    let check = || {
        let mut held: i128 = 0;
        for (_, key) in &all {
            let voucher = s.splitter.voucher(key).unwrap();
            if voucher.open {
                held += voucher.value;
            }
        }
        if let Some(pool) = s.splitter.pool(&sponsor, &s.publisher) {
            held += pool.balance;
        }
        assert_eq!(s.usdc.balance(&s.splitter.address), held);
    };
    check();

    s.splitter
        .fund_pool(&sponsor, &s.publisher, &operator, &PRICE, &(2 * PRICE));
    let reader = Address::generate(&s.env);
    s.redeem(&bought.keys[0], &reader, &paper(&s.env)).unwrap();
    check();
    s.splitter.pay_sponsored(
        &operator,
        &sponsor,
        &reader,
        &paper(&s.env),
        &s.publisher,
        &PRICE,
    );
    s.redeem(&other.keys[0], &reader, &paper(&s.env)).unwrap();
    check();

    s.advance(7 * DAY);
    s.splitter
        .reclaim_vouchers(&bought.buyer, &s.public_keys(&bought.keys[1..2]));
    check();
    assert_eq!(
        s.usdc.balance(&s.splitter.address),
        2 * PRICE,
        "queda un vale vencido sin devolver y el resto del fondo"
    );
    assert_eq!(s.usdc.balance(&s.publisher), 3 * 4_900_000);
    assert_eq!(s.usdc.balance(&s.treasury), 3 * 100_000);
}
