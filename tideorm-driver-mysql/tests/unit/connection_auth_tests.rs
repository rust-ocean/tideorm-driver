use super::*;
use bytes::Buf;

/// The server checks a scramble by XOR-ing it with a pad made from the nonce
/// and the stored `stage2 = H(H(password))`: the result must be
/// `stage1 = H(password)`. `mysql_native_password` hashes the nonce first and
/// SHA-1; `caching_sha2_password` hashes `stage2` first and SHA-256.
fn assert_server_recovers_stage1<D: Digest>(
    scramble: fn(&str, &Chain<Bytes, Bytes>) -> Vec<u8>,
    nonce_first: bool,
    password: &str,
    (a, b): (&'static [u8], &'static [u8]),
) {
    let stage1 = D::digest(password);
    let stage2 = D::digest(&stage1);
    let pad = if nonce_first {
        D::new()
            .chain_update(a)
            .chain_update(b)
            .chain_update(&stage2)
    } else {
        D::new()
            .chain_update(&stage2)
            .chain_update(a)
            .chain_update(b)
    }
    .finalize();

    let nonce = Bytes::from_static(a).chain(Bytes::from_static(b));
    let mut unscrambled = scramble(password, &nonce);
    xor_eq(&mut unscrambled, &pad);
    assert_eq!(unscrambled, &stage1[..], "{password:?}, {a:?} + {b:?}");
}

#[test]
fn the_server_recovers_the_password_hash_from_each_scramble() {
    let nonces: [(&'static [u8], &'static [u8]); 2] =
        [(b"0123456789", &[0xAB; 10]), (b"abcdefgh", b"ijklmnopqrst")];

    for password in ["my_pwd", "", "pass word with ünïcode"] {
        for nonce in nonces {
            assert_server_recovers_stage1::<Sha1>(scramble_sha1, true, password, nonce);
            assert_server_recovers_stage1::<Sha256>(scramble_sha256, false, password, nonce);
        }
    }
}

#[test]
fn an_empty_password_gets_an_empty_answer() {
    // An account without a password refuses a scramble of the empty password, in the
    // first response and after a switch alike.
    driver_core::rt::test_block_on(async {
        let socket = driver_core::net::ScriptedSocket::new(Vec::new());
        let mut stream =
            MySqlStream::with_socket(&crate::MySqlConnectOptions::new(), socket).boxed_socket();
        let nonce = Bytes::from_static(b"0123456789").chain(Bytes::from_static(b"abcdefghij"));
        for plugin in [
            AuthPlugin::MySqlNativePassword,
            AuthPlugin::CachingSha2Password,
        ] {
            let answer = plugin.scramble(&mut stream, "", &nonce).await.unwrap();
            assert!(answer.is_empty(), "{plugin:?}: {answer:?}");
        }
        // Without TLS, `sha256_password` sends an empty password as it is, with no key exchange.
        let answer = AuthPlugin::Sha256Password
            .scramble(&mut stream, "", &nonce)
            .await
            .unwrap();
        assert_eq!(answer, [0]);
    });
}
