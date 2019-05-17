use std::convert::TryInto;
use std::ffi::{CStr, CString};
use std::io::Cursor;

use pgp::composed::{
    Deserializable, KeyType as PgpKeyType, Message, SecretKeyParamsBuilder, SignedPublicKey,
    SignedSecretKey, SubkeyParamsBuilder,
};
use pgp::crypto::{HashAlgorithm, SymmetricKeyAlgorithm};
use pgp::types::{CompressionAlgorithm, KeyTrait, SecretKeyTrait, StringToKey};
use rand::thread_rng;
use sha2::{Digest, Sha256};

use crate::dc_hash::*;
use crate::dc_key::*;
use crate::dc_keyring::*;
use crate::dc_tools::*;
use crate::types::*;
use crate::x::*;

// TODO should return bool /rtn
pub unsafe fn dc_split_armored_data(
    buf: *mut libc::c_char,
    ret_headerline: *mut *const libc::c_char,
    ret_setupcodebegin: *mut *const libc::c_char,
    ret_preferencrypt: *mut *const libc::c_char,
    ret_base64: *mut *const libc::c_char,
) -> libc::c_int {
    let mut success: libc::c_int = 0i32;
    let mut line_chars: size_t = 0i32 as size_t;
    let mut line: *mut libc::c_char = buf;
    let mut p1: *mut libc::c_char = buf;
    let mut p2: *mut libc::c_char;
    let mut headerline: *mut libc::c_char = 0 as *mut libc::c_char;
    let mut base64: *mut libc::c_char = 0 as *mut libc::c_char;
    if !ret_headerline.is_null() {
        *ret_headerline = 0 as *const libc::c_char
    }
    if !ret_setupcodebegin.is_null() {
        *ret_setupcodebegin = 0 as *const libc::c_char
    }
    if !ret_preferencrypt.is_null() {
        *ret_preferencrypt = 0 as *const libc::c_char
    }
    if !ret_base64.is_null() {
        *ret_base64 = 0 as *const libc::c_char
    }
    if !(buf.is_null() || ret_headerline.is_null()) {
        dc_remove_cr_chars(buf);
        while 0 != *p1 {
            if *p1 as libc::c_int == '\n' as i32 {
                *line.offset(line_chars as isize) = 0i32 as libc::c_char;
                if headerline.is_null() {
                    dc_trim(line);
                    if strncmp(
                        line,
                        b"-----BEGIN \x00" as *const u8 as *const libc::c_char,
                        1,
                    ) == 0i32
                        && strncmp(
                            &mut *line.offset(strlen(line).wrapping_sub(5) as isize),
                            b"-----\x00" as *const u8 as *const libc::c_char,
                            5,
                        ) == 0i32
                    {
                        headerline = line;
                        if !ret_headerline.is_null() {
                            *ret_headerline = headerline
                        }
                    }
                } else if strspn(line, b"\t\r\n \x00" as *const u8 as *const libc::c_char)
                    == strlen(line)
                {
                    base64 = p1.offset(1isize);
                    break;
                } else {
                    p2 = strchr(line, ':' as i32);
                    if p2.is_null() {
                        *line.offset(line_chars as isize) = '\n' as i32 as libc::c_char;
                        base64 = line;
                        break;
                    } else {
                        *p2 = 0i32 as libc::c_char;
                        dc_trim(line);
                        if strcasecmp(
                            line,
                            b"Passphrase-Begin\x00" as *const u8 as *const libc::c_char,
                        ) == 0i32
                        {
                            p2 = p2.offset(1isize);
                            dc_trim(p2);
                            if !ret_setupcodebegin.is_null() {
                                *ret_setupcodebegin = p2
                            }
                        } else if strcasecmp(
                            line,
                            b"Autocrypt-Prefer-Encrypt\x00" as *const u8 as *const libc::c_char,
                        ) == 0i32
                        {
                            p2 = p2.offset(1isize);
                            dc_trim(p2);
                            if !ret_preferencrypt.is_null() {
                                *ret_preferencrypt = p2
                            }
                        }
                    }
                }
                p1 = p1.offset(1isize);
                line = p1;
                line_chars = 0i32 as size_t
            } else {
                p1 = p1.offset(1isize);
                line_chars = line_chars.wrapping_add(1)
            }
        }
        if !(headerline.is_null() || base64.is_null()) {
            /* now, line points to beginning of base64 data, search end */
            /*the trailing space makes sure, this is not a normal base64 sequence*/
            p1 = strstr(base64, b"-----END \x00" as *const u8 as *const libc::c_char);
            if !(p1.is_null()
                || strncmp(
                    p1.offset(9isize),
                    headerline.offset(11isize),
                    strlen(headerline.offset(11isize)),
                ) != 0i32)
            {
                *p1 = 0i32 as libc::c_char;
                dc_trim(base64);
                if !ret_base64.is_null() {
                    *ret_base64 = base64
                }
                success = 1i32
            }
        }
    }

    success
}

/// Create a new key pair.
pub fn dc_pgp_create_keypair(addr: *const libc::c_char) -> Option<(Key, Key)> {
    let user_id = format!("<{}>", unsafe { CStr::from_ptr(addr).to_str().unwrap() });

    let key_params = SecretKeyParamsBuilder::default()
        .key_type(PgpKeyType::Rsa(2048))
        .can_create_certificates(true)
        .can_sign(true)
        .primary_user_id(user_id.into())
        .passphrase(None)
        .preferred_symmetric_algorithms(smallvec![
            SymmetricKeyAlgorithm::AES256,
            SymmetricKeyAlgorithm::AES192,
            SymmetricKeyAlgorithm::AES128,
        ])
        .preferred_hash_algorithms(smallvec![
            HashAlgorithm::SHA2_256,
            HashAlgorithm::SHA2_384,
            HashAlgorithm::SHA2_512,
            HashAlgorithm::SHA2_224,
            HashAlgorithm::SHA1,
        ])
        .preferred_compression_algorithms(smallvec![
            CompressionAlgorithm::ZLIB,
            CompressionAlgorithm::ZIP,
        ])
        .subkey(
            SubkeyParamsBuilder::default()
                .key_type(PgpKeyType::Rsa(2048))
                .can_encrypt(true)
                .passphrase(None)
                .build()
                .unwrap(),
        )
        .build()
        .expect("invalid key params");

    let key = key_params.generate().expect("invalid params");
    let private_key = key.sign(|| "".into()).expect("failed to sign secret key");

    let public_key = private_key.public_key();
    let public_key = public_key
        .sign(&private_key, || "".into())
        .expect("failed to sign public key");

    private_key.verify().expect("invalid private key generated");
    public_key.verify().expect("invalid public key generated");

    Some((Key::Public(public_key), Key::Secret(private_key)))
}

pub fn dc_pgp_pk_encrypt(
    plain_text: *const libc::c_void,
    plain_bytes: size_t,
    public_keys_for_encryption: &Keyring,
    private_key_for_signing: Option<&Key>,
) -> Option<String> {
    assert!(!plain_text.is_null() && !plain_bytes > 0, "invalid input");

    let bytes = unsafe { std::slice::from_raw_parts(plain_text as *const u8, plain_bytes) };
    let lit_msg = Message::new_literal_bytes("", bytes);
    let pkeys: Vec<&SignedPublicKey> = public_keys_for_encryption
        .keys()
        .into_iter()
        .filter_map(|key| {
            let k: &Key = &key;
            k.try_into().ok()
        })
        .collect();

    let mut rng = thread_rng();

    // TODO: measure time
    // TODO: better error handling
    let encrypted_msg = if let Some(private_key) = private_key_for_signing {
        let skey: &SignedSecretKey = private_key.try_into().unwrap();

        lit_msg
            .sign(skey, || "".into(), Default::default())
            .and_then(|msg| msg.compress(CompressionAlgorithm::ZLIB))
            .and_then(|msg| msg.encrypt_to_keys(&mut rng, Default::default(), &pkeys))
    } else {
        lit_msg.encrypt_to_keys(&mut rng, Default::default(), &pkeys)
    };

    encrypted_msg
        .and_then(|msg| msg.to_armored_string(None))
        .ok()
}

pub fn dc_pgp_pk_decrypt(
    ctext: *const libc::c_void,
    ctext_bytes: size_t,
    private_keys_for_decryption: &Keyring,
    public_keys_for_validation: &Keyring,
    ret_signature_fingerprints: *mut dc_hash_t,
) -> Option<Vec<u8>> {
    assert!(!ctext.is_null() && ctext_bytes > 0, "invalid input");

    let ctext = unsafe { std::slice::from_raw_parts(ctext as *const u8, ctext_bytes) };

    // TODO: proper error handling
    if let Ok((msg, _)) = Message::from_armor_single(Cursor::new(ctext)) {
        let skeys: Vec<&SignedSecretKey> = private_keys_for_decryption
            .keys()
            .iter()
            .filter_map(|key| {
                let k: &Key = &key;
                k.try_into().ok()
            })
            .collect();

        msg.decrypt(|| "".into(), || "".into(), &skeys[..])
            .and_then(|(mut decryptor, _)| {
                // TODO: how to handle the case when we detect multiple messages?
                decryptor.next().expect("no message")
            })
            .and_then(|dec_msg| {
                if !ret_signature_fingerprints.is_null()
                    && !public_keys_for_validation.keys().is_empty()
                {
                    let pkeys: Vec<&SignedPublicKey> = public_keys_for_validation
                        .keys()
                        .iter()
                        .filter_map(|key| {
                            let k: &Key = &key;
                            k.try_into().ok()
                        })
                        .collect();

                    for pkey in &pkeys {
                        if dec_msg.verify(&pkey.primary_key).is_ok() {
                            let fp_r = hex::encode_upper(pkey.fingerprint());
                            let len = fp_r.len() as libc::c_int;
                            let fp_c = CString::new(fp_r).unwrap();
                            let fp = unsafe { strdup(fp_c.as_ptr()) };

                            unsafe {
                                dc_hash_insert(
                                    ret_signature_fingerprints,
                                    fp as *const _,
                                    len,
                                    1 as *mut _,
                                )
                            };
                        }
                    }
                }
                dec_msg.get_content()
            })
            .ok()
            .and_then(|content| content)
    } else {
        None
    }
}

/// Symmetric encryption.
pub fn dc_pgp_symm_encrypt(
    passphrase: *const libc::c_char,
    plain: *const libc::c_void,
    plain_bytes: size_t,
) -> Option<String> {
    assert!(!passphrase.is_null(), "invalid passphrase");
    assert!(!plain.is_null() && !plain_bytes > 0, "invalid input");

    let pw = unsafe { CStr::from_ptr(passphrase).to_str().unwrap() };
    let bytes = unsafe { std::slice::from_raw_parts(plain as *const u8, plain_bytes) };

    let mut rng = thread_rng();
    let lit_msg = Message::new_literal_bytes("", bytes);

    let s2k = StringToKey::new_default(&mut rng);
    let msg = lit_msg.encrypt_with_password(&mut rng, s2k, Default::default(), || pw.into());

    msg.and_then(|msg| msg.to_armored_string(None)).ok()
}

/// Symmetric decryption.
pub fn dc_pgp_symm_decrypt(
    passphrase: *const libc::c_char,
    ctext: *const libc::c_void,
    ctext_bytes: size_t,
) -> Option<Vec<u8>> {
    assert!(!passphrase.is_null(), "invalid passphrase");
    assert!(!ctext.is_null() && !ctext_bytes > 0, "invalid input");

    let pw = unsafe { CStr::from_ptr(passphrase).to_str().unwrap() };
    let bytes = unsafe { std::slice::from_raw_parts(ctext as *const u8, ctext_bytes) };

    let enc_msg = Message::from_bytes(Cursor::new(bytes));

    enc_msg
        .and_then(|msg| {
            let mut decryptor = msg
                .decrypt_with_password(|| pw.into())
                .expect("failed decryption");
            decryptor.next().expect("no message")
        })
        .and_then(|msg| msg.get_content())
        .ok()
        .and_then(|content| content)
}

/// Calculate the SHA256 hash of the given bytes.
pub fn dc_hash_sha256(bytes_ptr: *const u8, bytes_len: libc::size_t) -> (*mut u8, libc::size_t) {
    assert!(!bytes_ptr.is_null());
    assert!(bytes_len > 0);

    let bytes = unsafe { std::slice::from_raw_parts(bytes_ptr, bytes_len) };
    let result = Sha256::digest(bytes);

    let mut r = result.to_vec();
    r.shrink_to_fit();

    let ptr = r.as_ptr();
    let len = r.len();
    std::mem::forget(r);

    (ptr as *mut _, len)
}
