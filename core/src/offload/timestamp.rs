//! Horodatage certifié RFC 3161 du rapport PDF (option, poste connecté).
//!
//! L'empreinte SHA-256 du PDF est envoyée à une autorité d'horodatage (TSA),
//! qui renvoie un jeton signé : preuve que le rapport existait sous cette
//! forme à cette date. Le jeton est écrit à côté du PDF (`.pdf.tsr`) et se
//! vérifie avec OpenSSL :
//! `openssl ts -verify -in rapport.pdf.tsr -data rapport.pdf -CAfile <chaîne de la TSA>`.
//!
//! Le jeton est signé par l'autorité : HTTP suffit (pas de TLS nécessaire).
//! VERIFLOW contrôle le statut, l'empreinte et le nonce de la réponse, pas la
//! signature elle-même (rôle d'OpenSSL ou d'Acrobat).

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Autorité par défaut : DigiCert (gratuite, reconnue par Acrobat).
pub const DEFAULT_TSA: &str = "http://timestamp.digicert.com";

/// Jeton reçu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    /// Réponse complète (TimeStampResp, DER), écrite telle quelle en `.tsr`.
    pub der: Vec<u8>,
    /// Date certifiée (GeneralizedTime, UTC), « 20261020184652Z ».
    pub time: String,
}

/// Requête TimeStampReq (DER) pour une empreinte SHA-256, avec nonce et
/// demande du certificat de la TSA.
pub fn request(sha256: &[u8; 32], nonce: u64) -> Vec<u8> {
    // AlgorithmIdentifier sha256 (2.16.840.1.101.3.4.2.1), paramètres NULL.
    let alg = [
        0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05, 0x00,
    ];
    let mut imprint = alg.to_vec();
    imprint.extend([0x04, 0x20]);
    imprint.extend(sha256);
    let mut body = vec![0x02, 0x01, 0x01]; // version 1
    body.extend(tlv(0x30, &imprint));
    if nonce != 0 {
        body.extend(tlv(0x02, &integer(nonce)));
    }
    body.extend([0x01, 0x01, 0xff]); // certReq TRUE
    tlv(0x30, &body)
}

/// Entier DER positif (octet nul en tête si le bit de poids fort est à 1).
fn integer(v: u64) -> Vec<u8> {
    let bytes = v.to_be_bytes();
    let first = bytes.iter().position(|&b| b != 0).unwrap_or(7);
    let mut out = bytes[first..].to_vec();
    if out[0] & 0x80 != 0 {
        out.insert(0, 0);
    }
    out
}

fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let n = content.len();
    if n < 0x80 {
        out.push(n as u8);
    } else {
        let len = (n as u32).to_be_bytes();
        let first = len.iter().position(|&b| b != 0).unwrap_or(3);
        out.push(0x80 | (4 - first) as u8);
        out.extend(&len[first..]);
    }
    out.extend(content);
    out
}

/// Lit un élément DER : (étiquette, contenu, reste).
fn read(der: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let tag = *der.first()?;
    let first = *der.get(1)?;
    let (len, start) = if first < 0x80 {
        (first as usize, 2)
    } else {
        let n = (first & 0x7f) as usize;
        if n == 0 || n > 4 {
            return None;
        }
        let mut len = 0usize;
        for b in der.get(2..2 + n)? {
            len = (len << 8) | *b as usize;
        }
        (len, 2 + n)
    };
    let end = start.checked_add(len)?;
    Some((tag, der.get(start..end)?, der.get(end..)?))
}

/// Contrôle une réponse TimeStampResp : statut accordé, empreinte et nonce
/// identiques à ceux de la requête. Renvoie la date certifiée.
pub fn check(resp: &[u8], sha256: &[u8; 32], nonce: u64) -> Result<String, String> {
    let (tag, body, _) = read(resp).ok_or("réponse illisible")?;
    if tag != 0x30 {
        return Err("réponse illisible".into());
    }
    // PKIStatusInfo : statut 0 (accordé) ou 1 (accordé avec modifications).
    let (_, status_info, token) = read(body).ok_or("statut illisible")?;
    let (_, status, _) = read(status_info).ok_or("statut illisible")?;
    match status {
        [0] | [1] => {}
        [s] => return Err(format!("refusé par l'autorité (statut {s})")),
        _ => return Err("statut illisible".into()),
    }
    if token.is_empty() {
        return Err("jeton absent".into());
    }
    // L'empreinte figure telle quelle dans le TSTInfo (messageImprint).
    if !token.windows(32).any(|w| w == sha256) {
        return Err("le jeton ne porte pas l'empreinte du rapport".into());
    }
    if nonce != 0 {
        let n = tlv(0x02, &integer(nonce));
        if !token.windows(n.len()).any(|w| w == n.as_slice()) {
            return Err("nonce différent de celui de la requête".into());
        }
    }
    gen_time(token).ok_or_else(|| "date du jeton illisible".into())
}

/// Première date GeneralizedTime du jeton : genTime du TSTInfo (la date de
/// signature CMS, plus loin, est en UTCTime).
fn gen_time(token: &[u8]) -> Option<String> {
    let mut i = 0;
    while i + 2 < token.len() {
        if token[i] == 0x18 {
            let len = token[i + 1] as usize;
            if (15..=24).contains(&len) {
                if let Some(s) = token.get(i + 2..i + 2 + len) {
                    if s.last() == Some(&b'Z') && s[..14].iter().all(u8::is_ascii_digit) {
                        return String::from_utf8(s.to_vec()).ok();
                    }
                }
            }
        }
        i += 1;
    }
    None
}

/// Demande un jeton à l'autorité `url` (15 s au plus).
pub fn stamp(url: &str, sha256: &[u8; 32]) -> Result<Token, String> {
    // Nonce : horloge en nanosecondes, suffisant contre le rejeu.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
        | 1;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    let mut resp = agent
        .post(url)
        .header("Content-Type", "application/timestamp-query")
        .send(&request(sha256, nonce)[..])
        .map_err(|e| format!("autorité injoignable ({e})"))?;
    let der = resp
        .body_mut()
        .with_config()
        .limit(1 << 20)
        .read_to_vec()
        .map_err(|e| format!("réponse incomplète ({e})"))?;
    let time = check(&der, sha256, nonce)?;
    Ok(Token { der, time })
}

/// « 20261020184652Z » vers « 2026-10-20 18:46:52 UTC ».
pub fn display_time(t: &str) -> String {
    if t.len() < 14 {
        return t.to_owned();
    }
    format!(
        "{}-{}-{} {}:{}:{} UTC",
        &t[0..4],
        &t[4..6],
        &t[6..8],
        &t[8..10],
        &t[10..12],
        &t[12..14]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: [u8; 32] = [
        0x9f, 0x86, 0xd0, 0x81, 0x88, 0x4c, 0x7d, 0x65, 0x9a, 0x2f, 0xea, 0xa0, 0xc5, 0x5a, 0xd0,
        0x15, 0xa3, 0xbf, 0x4f, 0x1b, 0x2b, 0x0b, 0x82, 0x2c, 0xd1, 0x5d, 0x6c, 0x15, 0xb0, 0xf0,
        0x0a, 0x08,
    ];

    #[test]
    fn request_matches_openssl() {
        // `openssl ts -query -data t.txt -sha256 -cert -no_nonce` (t.txt = « test »).
        let mut expected = vec![
            0x30, 0x39, 0x02, 0x01, 0x01, 0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48,
            0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05, 0x00, 0x04, 0x20,
        ];
        expected.extend(HASH);
        expected.extend([0x01, 0x01, 0xff]);
        assert_eq!(request(&HASH, 0), expected);
        // Avec nonce : entier ajouté, longueur totale mise à jour.
        let r = request(&HASH, 0x8A37_BF75_A595_DB91);
        assert_eq!(r[1] as usize, r.len() - 2);
        assert!(r
            .windows(11)
            .any(|w| w == [0x02, 0x09, 0x00, 0x8a, 0x37, 0xbf, 0x75, 0xa5, 0x95, 0xdb, 0x91]));
    }

    /// Réponse minimale : statut, puis un « jeton » contenant l'empreinte,
    /// le nonce et la date.
    fn fake_response(status: u8, hash: &[u8; 32], nonce: u64) -> Vec<u8> {
        let status_info = tlv(0x30, &tlv(0x02, &[status]));
        let mut tst = tlv(0x04, hash);
        tst.extend(tlv(0x02, &integer(nonce)));
        tst.extend(tlv(0x18, b"20261020184652Z"));
        let token = tlv(0x30, &tst);
        let mut body = status_info;
        body.extend(token);
        tlv(0x30, &body)
    }

    #[test]
    fn checks_response() {
        let ok = fake_response(0, &HASH, 42);
        assert_eq!(check(&ok, &HASH, 42).unwrap(), "20261020184652Z");
        assert!(check(&fake_response(2, &HASH, 42), &HASH, 42)
            .unwrap_err()
            .contains("refusé"));
        let mut other = HASH;
        other[0] ^= 1;
        assert!(check(&ok, &other, 42).unwrap_err().contains("empreinte"));
        assert!(check(&ok, &HASH, 43).unwrap_err().contains("nonce"));
        assert!(check(b"\x30", &HASH, 42).is_err());
        assert_eq!(display_time("20261020184652Z"), "2026-10-20 18:46:52 UTC");
    }

    /// Essai réel auprès de DigiCert : seulement si VERIFLOW_TSA_TEST est défini
    /// (réseau requis).
    #[test]
    fn real_authority() {
        if std::env::var_os("VERIFLOW_TSA_TEST").is_none() {
            return;
        }
        let t = stamp(DEFAULT_TSA, &HASH).unwrap();
        assert!(t.time.starts_with("20"), "{}", t.time);
        assert!(t.der.len() > 1000);
    }
}
