//! Empreintes de vérification (charte §7.1) : XXH64, XXH3, XXH128, MD5,
//! SHA-1, SHA-256 et C4, calculées en une seule lecture des données.
//!
//! Les formats textuels sont identiques à ceux de l'implémentation de
//! référence ASC MHL (hexadécimal canonique, C4 en base 58 sur 90 caractères).

use std::fmt;
use std::str::FromStr;

use md5::Digest;
use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3::Xxh3;
use xxhash_rust::xxh64::Xxh64;

/// Algorithmes disponibles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HashAlgo {
    Xxh128,
    Xxh64,
    Xxh3,
    Md5,
    Sha1,
    Sha256,
    C4,
}

impl HashAlgo {
    pub const ALL: [HashAlgo; 7] = [
        HashAlgo::Xxh128,
        HashAlgo::Xxh64,
        HashAlgo::Xxh3,
        HashAlgo::Md5,
        HashAlgo::Sha1,
        HashAlgo::Sha256,
        HashAlgo::C4,
    ];

    /// Identifiant (également nom d'élément XML ASC MHL quand il existe).
    pub fn id(self) -> &'static str {
        match self {
            HashAlgo::Xxh128 => "xxh128",
            HashAlgo::Xxh64 => "xxh64",
            HashAlgo::Xxh3 => "xxh3",
            HashAlgo::Md5 => "md5",
            HashAlgo::Sha1 => "sha1",
            HashAlgo::Sha256 => "sha256",
            HashAlgo::C4 => "c4",
        }
    }

    /// Nom affiché.
    pub fn label(self) -> &'static str {
        match self {
            HashAlgo::Xxh128 => "XXH128",
            HashAlgo::Xxh64 => "XXH64",
            HashAlgo::Xxh3 => "XXH3-64",
            HashAlgo::Md5 => "MD5",
            HashAlgo::Sha1 => "SHA-1",
            HashAlgo::Sha256 => "SHA-256",
            HashAlgo::C4 => "C4",
        }
    }

    /// Vrai si le format figure dans la norme ASC MHL v2 (SHA-256 n'y est pas).
    pub fn in_mhl(self) -> bool {
        self != HashAlgo::Sha256
    }

    /// Ordre des éléments imposé par le schéma XSD ASC MHL v2.
    pub fn mhl_order(self) -> u8 {
        match self {
            HashAlgo::C4 => 0,
            HashAlgo::Md5 => 1,
            HashAlgo::Sha1 => 2,
            HashAlgo::Xxh128 => 3,
            HashAlgo::Xxh3 => 4,
            HashAlgo::Xxh64 => 5,
            HashAlgo::Sha256 => 6,
        }
    }
}

impl fmt::Display for HashAlgo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl FromStr for HashAlgo {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        HashAlgo::ALL
            .into_iter()
            .find(|a| a.id().eq_ignore_ascii_case(s) || a.label().eq_ignore_ascii_case(s))
            .ok_or_else(|| format!("algorithme inconnu : {s}"))
    }
}

enum State {
    Xxh128(Box<Xxh3>),
    Xxh64(Xxh64),
    Xxh3(Box<Xxh3>),
    Md5(md5::Md5),
    Sha1(sha1::Sha1),
    Sha256(sha2::Sha256),
    C4(sha2::Sha512),
}

impl State {
    fn new(algo: HashAlgo) -> Self {
        match algo {
            HashAlgo::Xxh128 => State::Xxh128(Box::default()),
            HashAlgo::Xxh64 => State::Xxh64(Xxh64::new(0)),
            HashAlgo::Xxh3 => State::Xxh3(Box::default()),
            HashAlgo::Md5 => State::Md5(md5::Md5::new()),
            HashAlgo::Sha1 => State::Sha1(sha1::Sha1::new()),
            HashAlgo::Sha256 => State::Sha256(sha2::Sha256::new()),
            HashAlgo::C4 => State::C4(sha2::Sha512::new()),
        }
    }

    fn update(&mut self, data: &[u8]) {
        match self {
            State::Xxh128(h) | State::Xxh3(h) => h.update(data),
            State::Xxh64(h) => h.update(data),
            State::Md5(h) => h.update(data),
            State::Sha1(h) => h.update(data),
            State::Sha256(h) => h.update(data),
            State::C4(h) => h.update(data),
        }
    }

    fn finish(self) -> String {
        match self {
            State::Xxh128(h) => format!("{:032x}", h.digest128()),
            State::Xxh3(h) => format!("{:016x}", h.digest()),
            State::Xxh64(h) => format!("{:016x}", h.digest()),
            State::Md5(h) => hex(&h.finalize()),
            State::Sha1(h) => hex(&h.finalize()),
            State::Sha256(h) => hex(&h.finalize()),
            State::C4(h) => c4_encode(&h.finalize()),
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

const C4_ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Encode un condensé SHA-512 en identifiant C4 (« c4 » + 88 caractères base 58).
fn c4_encode(sha512: &[u8]) -> String {
    let mut num = sha512.to_vec();
    let mut digits = Vec::with_capacity(88);
    while num.iter().any(|&b| b != 0) {
        let mut rem = 0u32;
        for byte in num.iter_mut() {
            let acc = (rem << 8) | *byte as u32;
            *byte = (acc / 58) as u8;
            rem = acc % 58;
        }
        digits.push(C4_ALPHABET[rem as usize]);
    }
    while digits.len() < 88 {
        digits.push(b'1');
    }
    digits.reverse();
    format!("c4{}", String::from_utf8(digits).expect("alphabet ASCII"))
}

/// Décode un identifiant C4 en 64 octets.
fn c4_decode(s: &str) -> Option<Vec<u8>> {
    let body = s.strip_prefix("c4")?;
    let mut num = vec![0u8; 64];
    for ch in body.bytes() {
        let digit = C4_ALPHABET.iter().position(|&c| c == ch)? as u32;
        let mut carry = digit;
        for byte in num.iter_mut().rev() {
            let acc = *byte as u32 * 58 + carry;
            *byte = acc as u8;
            carry = acc >> 8;
        }
        if carry != 0 {
            return None;
        }
    }
    Some(num)
}

/// Octets représentés par une empreinte textuelle (pour les empreintes de dossiers).
pub fn digest_bytes(algo: HashAlgo, digest: &str) -> Option<Vec<u8>> {
    if algo == HashAlgo::C4 {
        return c4_decode(digest);
    }
    if digest.len() % 2 != 0 {
        return None;
    }
    (0..digest.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&digest[i..i + 2], 16).ok())
        .collect()
}

/// Calcul simultané de plusieurs empreintes sur un même flux de données.
pub struct MultiHasher {
    states: Vec<(HashAlgo, State)>,
}

impl MultiHasher {
    pub fn new(algos: &[HashAlgo]) -> Self {
        Self {
            states: algos.iter().map(|&a| (a, State::new(a))).collect(),
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        for (_, s) in &mut self.states {
            s.update(data);
        }
    }

    pub fn finish(self) -> Vec<(HashAlgo, String)> {
        self.states
            .into_iter()
            .map(|(a, s)| (a, s.finish()))
            .collect()
    }
}

/// Empreinte de données en mémoire.
pub fn hash_data(algo: HashAlgo, data: &[u8]) -> String {
    let mut s = State::new(algo);
    s.update(data);
    s.finish()
}

/// Empreinte d'une liste d'empreintes (règle ASC MHL : tri lexicographique des
/// chaînes, puis concaténation de leurs octets).
pub fn hash_of_hash_list(algo: HashAlgo, hashes: &[String]) -> String {
    let mut sorted: Vec<&String> = hashes.iter().collect();
    sorted.sort();
    let mut s = State::new(algo);
    for h in sorted {
        s.update(&digest_bytes(algo, h).unwrap_or_default());
    }
    s.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Vecteurs produits par l'implémentation de référence ascmhl 1.2 (ASC).
    const DATA: &[u8] = b"VERIFLOW test\n";

    #[test]
    fn matches_ascmhl_reference_vectors() {
        let cases = [
            (HashAlgo::Xxh64, "bbdca3b7529acde5"),
            (HashAlgo::Xxh3, "c5049f35726f174a"),
            (HashAlgo::Xxh128, "33d7d52c48b9b873bbee05dd9856a171"),
            (HashAlgo::Md5, "3fea88eaf665d874c7317176576dc383"),
            (HashAlgo::Sha1, "b02a037f38b92b919fd62c2fef5422716885763d"),
            (
                HashAlgo::C4,
                "c43Jtvradanh8SHNGcQuN5kYJrdvvjNia9EZeoUbv1SxUnJrKU8meV4LM2oQwuFj6HLFvKzsCCTPF1nfFaK37puBVN",
            ),
        ];
        for (algo, expected) in cases {
            assert_eq!(hash_data(algo, DATA), expected, "{algo}");
        }
        assert_eq!(
            hash_data(HashAlgo::C4, b""),
            "c459dsjfscH38cYeXXYogktxf4Cd9ibshE3BHUo6a58hBXmRQdZrAkZzsWcbWtDg5oQstpDuni4Hirj75GEmTc1sFT"
        );
    }

    #[test]
    fn multi_hasher_equals_single() {
        let mut m = MultiHasher::new(&HashAlgo::ALL);
        m.update(&DATA[..5]);
        m.update(&DATA[5..]);
        for (algo, digest) in m.finish() {
            assert_eq!(digest, hash_data(algo, DATA));
        }
    }

    #[test]
    fn hash_of_hash_lists_match_reference() {
        let ab = |algo| vec![hash_data(algo, b"b"), hash_data(algo, b"a")];
        assert_eq!(
            hash_of_hash_list(HashAlgo::Xxh128, &ab(HashAlgo::Xxh128)),
            "ed1a78187b198e63c6fdbdc99f53f0b4"
        );
        assert_eq!(
            hash_of_hash_list(HashAlgo::C4, &ab(HashAlgo::C4)),
            "c44HFJH3VrXdeCKFEZCnSHJk3HCp5u3tYey3fdpxaWvE2j6eo5GpoiUmRDraYAXtfUotnvBcA8KALugf2SXjhW7xUK"
        );
        assert_eq!(hash_of_hash_list(HashAlgo::Xxh64, &[]), "ef46db3751d8e999");
    }

    #[test]
    fn c4_roundtrip_and_parsing() {
        let id = hash_data(HashAlgo::C4, DATA);
        assert_eq!(id.len(), 90);
        assert_eq!(c4_encode(&c4_decode(&id).unwrap()), id);
        assert_eq!("XXH128".parse::<HashAlgo>().unwrap(), HashAlgo::Xxh128);
        assert_eq!("sha256".parse::<HashAlgo>().unwrap(), HashAlgo::Sha256);
        assert!(!HashAlgo::Sha256.in_mhl());
    }
}
