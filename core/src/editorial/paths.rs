//! Chemins de fichiers dans les exports : URL `file://` (FCPXML, OTIO, XMEML),
//! chemins relatifs et échappement des attributs XML. Tout se fait sur le
//! texte, sans accès au disque : un chemin Windows se traite aussi sous Linux.

/// Découpage d'un chemin en racine et segments.
#[derive(Debug, PartialEq)]
enum Root {
    /// Lecteur Windows (« C: »), lettre en majuscule.
    Drive(char),
    /// Partage réseau : serveur et nom du partage.
    Unc(String, String),
    /// Racine Unix (« / »).
    Unix,
    /// Chemin sans racine.
    None,
}

/// Retire le préfixe de chemin étendu de Windows : `\\?\C:\x` devient
/// `C:\x`, `\\?\UNC\srv\p` devient `\\srv\p`. Entrée avec « / ».
fn strip_extended(p: String) -> String {
    let head = |n: usize| p.get(..n).unwrap_or("");
    if head(8).eq_ignore_ascii_case("//?/UNC/") {
        format!("//{}", &p[8..])
    } else if head(4) == "//?/" {
        p[4..].to_owned()
    } else {
        p
    }
}

fn split(path: &str) -> (Root, Vec<String>) {
    let p = strip_extended(path.replace('\\', "/"));
    let parts = |s: &str| -> Vec<String> {
        s.split('/')
            .filter(|x| !x.is_empty() && *x != ".")
            .map(str::to_owned)
            .collect()
    };
    if let Some(rest) = p.strip_prefix("//") {
        let mut it = parts(rest).into_iter();
        let server = it.next().unwrap_or_default();
        let share = it.next().unwrap_or_default();
        return (Root::Unc(server, share), it.collect());
    }
    let b = p.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return (
            Root::Drive(b[0].to_ascii_uppercase() as char),
            parts(&p[2..]),
        );
    }
    if p.starts_with('/') {
        (Root::Unix, parts(&p))
    } else {
        (Root::None, parts(&p))
    }
}

/// Encodage pour cent UTF-8 d'un segment (la forme NFD de macOS est conservée).
pub(crate) fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out += &format!("%{b:02X}"),
        }
    }
    out
}

fn url(path: &str, prefix: &str, colon: &str) -> String {
    let (root, segs) = split(path);
    let enc: Vec<String> = segs.iter().map(|s| encode(s)).collect();
    let tail = enc.join("/");
    match root {
        Root::Drive(d) => format!("{prefix}/{d}{colon}/{tail}"),
        Root::Unc(server, share) => {
            let mut s = format!("file://{}/{}", encode(&server), encode(&share));
            if !tail.is_empty() {
                s += &format!("/{tail}");
            }
            s
        }
        Root::Unix | Root::None => format!("{prefix}/{tail}"),
    }
}

/// URL `file://` pour FCPXML et OTIO : `file:///C:/...`, `file:///Volumes/...`,
/// `file://serveur/partage/...` pour un partage réseau.
pub fn url_file(path: &str) -> String {
    url(path, "file://", ":")
}

/// URL `file://localhost/` du XMEML (forme écrite par Premiere, deux-points du
/// lecteur encodé) : `file://localhost/C%3A/...`. Un partage réseau garde
/// la forme `file://serveur/partage/...`.
pub fn url_xmeml(path: &str) -> String {
    url(path, "file://localhost", "%3A")
}

/// Chemin de `path` relatif au dossier `base`, avec des « / ». Si les deux
/// n'ont pas la même racine (autre lecteur, autre partage), le chemin absolu
/// est rendu tel quel (séparateurs « / »).
pub fn relative(path: &str, base: &str) -> String {
    let (rp, sp) = split(path);
    let (rb, sb) = split(base);
    let absolute = || {
        let tail = sp.join("/");
        match &rp {
            Root::Drive(d) => format!("{d}:/{tail}"),
            Root::Unc(s, sh) => format!("//{s}/{sh}/{tail}"),
            Root::Unix => format!("/{tail}"),
            Root::None => tail,
        }
    };
    // Windows ne distingue pas la casse des lecteurs et des partages.
    let same_root = match (&rp, &rb) {
        (Root::Unc(a, x), Root::Unc(b, y)) => {
            a.eq_ignore_ascii_case(b) && x.eq_ignore_ascii_case(y)
        }
        (a, b) => a == b,
    };
    if !same_root || rp == Root::None {
        return absolute();
    }
    let drive_like = !matches!(rp, Root::Unix);
    let eq = |a: &String, b: &String| {
        if drive_like {
            a.to_lowercase() == b.to_lowercase()
        } else {
            a == b
        }
    };
    let common = sp.iter().zip(&sb).take_while(|(a, b)| eq(a, b)).count();
    let mut out: Vec<String> = vec!["..".into(); sb.len() - common];
    out.extend(sp[common..].iter().cloned());
    out.join("/")
}

/// Texte pour une valeur d'attribut XML : caractères réservés échappés,
/// caractères de contrôle interdits en XML 1.0 retirés, retours à la ligne
/// conservés sous forme de référence.
pub fn xml_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out += "&amp;",
            '<' => out += "&lt;",
            '>' => out += "&gt;",
            '"' => out += "&quot;",
            '\'' => out += "&apos;",
            '\n' => out += "&#10;",
            '\r' => out += "&#13;",
            '\t' => out += "&#9;",
            c if (c as u32) < 0x20 || c == '\u{7f}' || c == '\u{fffe}' || c == '\u{ffff}' => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_windows_drive() {
        assert_eq!(
            url_file(r"C:\Rushes\A001.mov"),
            "file:///C:/Rushes/A001.mov"
        );
        assert_eq!(url_file("d:/x/y.wav"), "file:///D:/x/y.wav");
        assert_eq!(url_file("/Volumes/SSD/A.mov"), "file:///Volumes/SSD/A.mov");
        assert_eq!(
            url_xmeml(r"C:\Rushes\A001.mov"),
            "file://localhost/C%3A/Rushes/A001.mov"
        );
        assert_eq!(
            url_xmeml("/Volumes/SSD/A.mov"),
            "file://localhost/Volumes/SSD/A.mov"
        );
    }

    #[test]
    fn extended_prefix_removed() {
        assert_eq!(
            url_file(r"\\?\C:\Rushes\A001.mov"),
            "file:///C:/Rushes/A001.mov"
        );
        assert_eq!(
            url_xmeml(r"\\?\UNC\serveur\partage\a.mov"),
            "file://serveur/partage/a.mov"
        );
        assert_eq!(url_file(r"\\?\unc\srv\p\x y.mov"), "file://srv/p/x%20y.mov");
        assert_eq!(
            relative(r"\\?\C:\Prod\Rushes\x.mov", r"C:\Prod\Montage"),
            "../Rushes/x.mov"
        );
    }

    #[test]
    fn url_unc() {
        assert_eq!(
            url_file(r"\\serveur\partage\Mon dossier\a.mov"),
            "file://serveur/partage/Mon%20dossier/a.mov"
        );
        assert_eq!(
            url_xmeml(r"\\serveur\partage\a.mov"),
            "file://serveur/partage/a.mov"
        );
    }

    #[test]
    fn url_spaces_accents() {
        assert_eq!(
            url_file("/Users/léa/Plan 1 #2.mov"),
            "file:///Users/l%C3%A9a/Plan%201%20%232.mov"
        );
        // NFD (macOS) : « e » suivi de U+0301, conservé tel quel.
        assert_eq!(url_file("/a/e\u{301}.mov"), "file:///a/e%CC%81.mov");
    }

    #[test]
    fn relative_basic() {
        assert_eq!(relative("/a/b/c/x.mov", "/a/b"), "c/x.mov");
        assert_eq!(relative("/a/x.mov", "/a/b/c"), "../../x.mov");
        assert_eq!(
            relative(r"C:\Prod\Rushes\x.mov", r"c:\prod\Montage"),
            "../Rushes/x.mov"
        );
    }

    #[test]
    fn relative_other_drive_falls_back() {
        assert_eq!(
            relative(r"D:\Rushes\x.mov", r"C:\Montage"),
            "D:/Rushes/x.mov"
        );
        assert_eq!(relative(r"\\srv\p\x.mov", r"\\autre\p"), "//srv/p/x.mov");
        assert_eq!(relative("/Volumes/A/x.mov", r"C:\M"), "/Volumes/A/x.mov");
    }

    #[test]
    fn xml_attr_strips_control_chars() {
        assert_eq!(xml_attr("a\u{0}b\u{1b}c\u{7f}"), "abc");
        assert_eq!(xml_attr("l1\nl2\r\n"), "l1&#10;l2&#13;&#10;");
        assert_eq!(
            xml_attr(r#"<a href="x">&'"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&apos;"
        );
    }
}
