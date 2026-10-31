//! Sous-titres : lecture et écriture SRT, WebVTT et ASS/SSA, contrôles de
//! lisibilité.
//!
//! ASS : seules les lignes `Dialogue:` sont éditées (temps et texte, `\N`
//! devenant un saut de ligne). Les autres champs de chaque ligne (style,
//! calque, marges...) sont gardés avec le sous-titre ; le reste du fichier
//! (Script Info, styles, polices, commentaires) est recopié à l'identique.
//!
//! Les temps sont en secondes depuis le début du média. Les contrôles suivent
//! les usages courants du sous-titrage (42 caractères par ligne, 2 lignes,
//! 17 caractères par seconde, 5/6 s à 7 s par sous-titre, 2 images d'écart).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
    /// ASS : autres champs de la ligne `Dialogue:` (nom du champ en
    /// minuscules : layer, style, name, marginl...). Absent en SRT et WebVTT.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ass: Option<BTreeMap<String, String>>,
}

/// Lit un fichier .srt ou .vtt.
pub fn load(path: &Path) -> Result<Vec<Cue>> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_start_matches('\u{feff}');
    match ext(path).as_str() {
        "srt" | "vtt" => Ok(parse(text)),
        "ass" | "ssa" => Ok(parse_ass(text)),
        other => Err(Error::Unsupported(format!(
            "sous-titres .{other} : l'éditeur lit les fichiers SRT, WebVTT et ASS"
        ))),
    }
}

/// Enregistre en SRT, WebVTT ou ASS selon l'extension. En ASS, `template`
/// (le fichier ASS ouvert dans l'éditeur) fournit tout ce qui n'est pas une
/// ligne `Dialogue:` ; sans lui, en-tête et style par défaut.
pub fn save(path: &Path, cues: &[Cue], template: Option<&Path>) -> Result<()> {
    let content = match ext(path).as_str() {
        "vtt" => to_vtt(cues),
        "srt" => to_srt(cues),
        "ass" | "ssa" => {
            let base = template
                .filter(|t| matches!(ext(t).as_str(), "ass" | "ssa"))
                .and_then(|t| std::fs::read(t).ok())
                .map(|b| {
                    String::from_utf8_lossy(&b)
                        .trim_start_matches('\u{feff}')
                        .to_owned()
                });
            to_ass(cues, base.as_deref())
        }
        other => {
            return Err(Error::Unsupported(format!(
                "sous-titres .{other} : enregistrer en .srt, .vtt ou .ass"
            )))
        }
    };
    std::fs::write(path, content)?;
    Ok(())
}

/// Champs d'une ligne `Dialogue:` en ASS (v4+) quand le fichier n'a pas de
/// ligne `Format:`.
const ASS_FORMAT: [&str; 10] = [
    "layer", "start", "end", "style", "name", "marginl", "marginr", "marginv", "effect", "text",
];

/// Ligne `Format:` de la section [Events] (noms en minuscules).
fn ass_format(text: &str) -> Vec<String> {
    let mut in_events = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_events = l.eq_ignore_ascii_case("[events]");
        } else if in_events {
            if let Some(f) = l.strip_prefix("Format:") {
                return f.split(',').map(|s| s.trim().to_lowercase()).collect();
            }
        }
    }
    ASS_FORMAT.iter().map(|s| s.to_string()).collect()
}

/// Lit les lignes `Dialogue:` d'un fichier ASS ou SSA.
pub fn parse_ass(text: &str) -> Vec<Cue> {
    let format = ass_format(text);
    let mut cues = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim_start().strip_prefix("Dialogue:") else {
            continue;
        };
        // Le texte, dernier champ, peut contenir des virgules.
        let values: Vec<&str> = rest.trim_start().splitn(format.len(), ',').collect();
        if values.len() != format.len() {
            continue;
        }
        let mut fields = BTreeMap::new();
        let (mut start, mut end, mut body) = (None, None, String::new());
        for (name, value) in format.iter().zip(values) {
            match name.as_str() {
                "start" => start = parse_time(value),
                "end" => end = parse_time(value),
                "text" => body = value.replace("\\N", "\n"),
                _ => {
                    fields.insert(name.clone(), value.trim().to_owned());
                }
            }
        }
        if let (Some(start), Some(end)) = (start, end) {
            cues.push(Cue {
                start,
                end,
                text: body,
                ass: Some(fields),
            });
        }
    }
    cues
}

/// « H:MM:SS.cc » (centièmes de seconde, format ASS).
pub fn format_ass_time(seconds: f64) -> String {
    let cs = (seconds.max(0.0) * 100.0).round() as u64;
    format!(
        "{}:{:02}:{:02}.{:02}",
        cs / 360_000,
        cs / 6000 % 60,
        cs / 100 % 60,
        cs % 100
    )
}

/// En-tête d'un fichier ASS créé de zéro : 1920 × 1080, style « Default »
/// (Inter 64, blanc, contour noir, en bas au centre).
const ASS_HEADER: &str = "[Script Info]\nScriptType: v4.00+\nPlayResX: 1920\nPlayResY: 1080\nWrapStyle: 0\nScaledBorderAndShadow: yes\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Inter,64,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,3,1,2,60,60,60,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n";

/// Écrit un ASS : `base` (le fichier d'origine) privé de ses lignes
/// `Dialogue:`, qui sont remplacées par `cues` à la place de la première
/// (sinon en fin de section [Events]).
pub fn to_ass(cues: &[Cue], base: Option<&str>) -> String {
    let base = base.unwrap_or(ASS_HEADER);
    let format = ass_format(base);
    // Champs d'un sous-titre ajouté dans l'éditeur : ceux du premier
    // sous-titre ASS (style, calque, marges), sans nom ni effet.
    let mut fallback: BTreeMap<String, String> =
        cues.iter().find_map(|c| c.ass.clone()).unwrap_or_default();
    fallback.remove("name");
    fallback.remove("effect");
    let dialogue = |c: &Cue| -> String {
        let fields = c.ass.as_ref().unwrap_or(&fallback);
        let values: Vec<String> = format
            .iter()
            .map(|name| match name.as_str() {
                "start" => format_ass_time(c.start),
                "end" => format_ass_time(c.end),
                "text" => c.text.trim().replace("\r\n", "\n").replace('\n', "\\N"),
                "layer" | "marginl" | "marginr" | "marginv" => {
                    fields.get(name).cloned().unwrap_or_else(|| "0".into())
                }
                "style" => fields
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| "Default".into()),
                _ => fields.get(name).cloned().unwrap_or_default(),
            })
            .collect();
        format!("Dialogue: {}\n", values.join(","))
    };
    let lines: String = cues.iter().map(dialogue).collect();
    let mut out = String::new();
    let mut placed = false;
    let mut in_events = false;
    for line in base.replace("\r\n", "\n").lines() {
        let l = line.trim_start();
        if l.starts_with('[') {
            if in_events && !placed {
                out += &lines;
                out.push('\n');
                placed = true;
            }
            in_events = l.trim().eq_ignore_ascii_case("[events]");
        }
        if l.starts_with("Dialogue:") {
            if !placed {
                out += &lines;
                placed = true;
            }
            continue;
        }
        out += line;
        out.push('\n');
    }
    if !placed {
        if !in_events {
            out += "\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n";
        }
        out += &lines;
    }
    out
}

fn ext(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Analyse un texte SRT ou WebVTT : blocs séparés par une ligne vide, ligne
/// de temps « début --> fin » (paramètres WebVTT ignorés), puis le texte.
pub fn parse(text: &str) -> Vec<Cue> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut cues = Vec::new();
    for block in text.split("\n\n") {
        let lines: Vec<&str> = block.lines().collect();
        let Some(pos) = lines.iter().position(|l| l.contains("-->")) else {
            continue;
        };
        let mut parts = lines[pos].split("-->");
        let (Some(a), Some(b)) = (parts.next(), parts.next()) else {
            continue;
        };
        let b = b.split_whitespace().next().unwrap_or("");
        let (Some(start), Some(end)) = (parse_time(a.trim()), parse_time(b)) else {
            continue;
        };
        let body = lines[pos + 1..].join("\n").trim().to_owned();
        cues.push(Cue {
            start,
            end,
            text: body,
            ass: None,
        });
    }
    cues
}

/// « 01:02:03,456 », « 01:02:03.456 » ou « 02:03.456 » (WebVTT).
pub fn parse_time(s: &str) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    let parts: Vec<&str> = s.split(':').collect();
    let (h, m, sec) = match parts.as_slice() {
        [h, m, sec] => (h.parse::<f64>().ok()?, m.parse::<f64>().ok()?, *sec),
        [m, sec] => (0.0, m.parse::<f64>().ok()?, *sec),
        _ => return None,
    };
    let sec: f64 = sec.parse().ok()?;
    (m < 60.0 && sec < 60.0 && h >= 0.0).then_some(h * 3600.0 + m * 60.0 + sec)
}

/// « HH:MM:SS,mmm » (SRT) ou « HH:MM:SS.mmm » (WebVTT).
pub fn format_time(seconds: f64, separator: char) -> String {
    let ms = (seconds.max(0.0) * 1000.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02}{separator}{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

pub fn to_srt(cues: &[Cue]) -> String {
    let mut out = String::new();
    for (i, c) in cues.iter().enumerate() {
        out += &format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            format_time(c.start, ','),
            format_time(c.end, ','),
            c.text.trim()
        );
    }
    out
}

pub fn to_vtt(cues: &[Cue]) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for c in cues {
        out += &format!(
            "{} --> {}\n{}\n\n",
            format_time(c.start, '.'),
            format_time(c.end, '.'),
            c.text.trim()
        );
    }
    out
}

/// Règles de lisibilité.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    pub max_chars: usize,
    pub max_lines: usize,
    /// Caractères par seconde.
    pub max_cps: f64,
    pub min_duration: f64,
    pub max_duration: f64,
    /// Écart minimal entre deux sous-titres, en images.
    pub min_gap_frames: f64,
    pub fps: f64,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            max_chars: 42,
            max_lines: 2,
            max_cps: 17.0,
            min_duration: 5.0 / 6.0,
            max_duration: 7.0,
            min_gap_frames: 2.0,
            fps: 25.0,
        }
    }
}

/// Problème relevé sur un sous-titre.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Issue {
    /// Fin avant le début.
    Inverted,
    /// Chevauche le suivant.
    Overlap,
    /// Trop près du suivant (en images).
    Gap {
        frames: f64,
    },
    TooShort {
        seconds: f64,
    },
    TooLong {
        seconds: f64,
    },
    TooFast {
        cps: f64,
    },
    LineTooLong {
        chars: usize,
    },
    TooManyLines {
        lines: usize,
    },
    Empty,
}

/// Caractères par seconde (sans les sauts de ligne ni les balises).
pub fn cps(cue: &Cue) -> f64 {
    let chars = strip_tags(&cue.text).chars().filter(|c| *c != '\n').count();
    let d = cue.end - cue.start;
    if d > 0.0 {
        chars as f64 / d
    } else {
        f64::INFINITY
    }
}

/// Texte sans balises (`<i>`, `{\an8}`...).
fn strip_tags(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '<' | '{' => depth += 1,
            '>' | '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Problèmes de chaque sous-titre (dans l'ordre de la liste).
pub fn check(cues: &[Cue], rules: &Rules) -> Vec<Vec<Issue>> {
    let fps = if rules.fps > 0.0 { rules.fps } else { 25.0 };
    cues.iter()
        .enumerate()
        .map(|(i, c)| {
            let mut issues = Vec::new();
            let d = c.end - c.start;
            let plain = strip_tags(&c.text);
            if plain.trim().is_empty() {
                issues.push(Issue::Empty);
            }
            if d <= 0.0 {
                issues.push(Issue::Inverted);
            } else {
                if d < rules.min_duration - 1e-6 {
                    issues.push(Issue::TooShort { seconds: d });
                }
                if d > rules.max_duration + 1e-6 {
                    issues.push(Issue::TooLong { seconds: d });
                }
                let speed = cps(c);
                if speed > rules.max_cps + 1e-6 {
                    issues.push(Issue::TooFast { cps: speed });
                }
            }
            let lines: Vec<&str> = plain.lines().collect();
            if lines.len() > rules.max_lines {
                issues.push(Issue::TooManyLines { lines: lines.len() });
            }
            if let Some(chars) = lines
                .iter()
                .map(|l| l.chars().count())
                .max()
                .filter(|n| *n > rules.max_chars)
            {
                issues.push(Issue::LineTooLong { chars });
            }
            if let Some(next) = cues.get(i + 1) {
                let gap = next.start - c.end;
                if gap < -1e-6 {
                    issues.push(Issue::Overlap);
                } else if gap * fps < rules.min_gap_frames - 1e-6 {
                    issues.push(Issue::Gap { frames: gap * fps });
                }
            }
            issues
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRT: &str = "\u{feff}1\r\n00:00:01,000 --> 00:00:03,500\r\nBonjour.\r\n\r\n2\r\n00:00:03,520 --> 00:00:04,000\r\n<i>Ligne une</i>\r\nLigne deux\r\n\r\n";

    #[test]
    fn reads_srt_with_bom_and_crlf() {
        let cues = parse(SRT.trim_start_matches('\u{feff}'));
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start, 1.0);
        assert_eq!(cues[0].end, 3.5);
        assert_eq!(cues[1].text, "<i>Ligne une</i>\nLigne deux");
    }

    #[test]
    fn reads_webvtt_with_settings_and_short_times() {
        let vtt = "WEBVTT\n\nNOTE commentaire\n\nintro\n00:01.000 --> 00:02.500 align:start line:0\nSalut\n\n01:00:00.000 --> 01:00:01.000\nFin\n";
        let cues = parse(vtt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start, 1.0);
        assert_eq!(cues[0].end, 2.5);
        assert_eq!(cues[0].text, "Salut");
        assert_eq!(cues[1].start, 3600.0);
    }

    #[test]
    fn writes_srt_and_vtt_that_read_back() {
        let cues = parse(SRT.trim_start_matches('\u{feff}'));
        let srt = to_srt(&cues);
        assert!(srt.starts_with("1\n00:00:01,000 --> 00:00:03,500\nBonjour.\n\n2\n"));
        assert_eq!(parse(&srt), cues);
        let vtt = to_vtt(&cues);
        assert!(vtt.starts_with("WEBVTT\n\n00:00:01.000 --> 00:00:03.500\n"));
        assert_eq!(parse(&vtt), cues);
    }

    #[test]
    fn checks_readability() {
        let cues = vec![
            Cue {
                start: 0.0,
                end: 0.5,
                text: "Trop court".into(),
                ass: None,
            },
            Cue {
                start: 0.52,
                end: 1.52,
                text: "Une ligne vraiment beaucoup trop longue pour un sous-titre".into(),
                ass: None,
            },
            Cue {
                start: 1.4,
                end: 9.0,
                text: "a\nb\nc".into(),
                ass: None,
            },
        ];
        let issues = check(&cues, &Rules::default());
        assert!(issues[0].contains(&Issue::TooShort { seconds: 0.5 }));
        // 0,02 s d'écart à 25 i/s : une demi-image.
        assert!(
            matches!(issues[0].last(), Some(Issue::Gap { frames }) if (*frames - 0.5).abs() < 1e-6)
        );
        assert!(issues[1].iter().any(|i| matches!(i, Issue::TooFast { .. })));
        assert!(issues[1]
            .iter()
            .any(|i| matches!(i, Issue::LineTooLong { chars: 58 })));
        assert!(issues[1].contains(&Issue::Overlap));
        assert!(issues[2].contains(&Issue::TooManyLines { lines: 3 }));
        assert!(issues[2].iter().any(|i| matches!(i, Issue::TooLong { .. })));
    }

    #[test]
    fn tags_are_not_counted() {
        let c = Cue {
            start: 0.0,
            end: 1.0,
            text: "{\\an8}<i>abc</i>".into(),
            ass: None,
        };
        assert_eq!(cps(&c), 3.0);
    }

    const ASS: &str = "\u{feff}[Script Info]\r\n; Script de test\r\nScriptType: v4.00+\r\nPlayResX: 1920\r\nPlayResY: 1080\r\n\r\n[V4+ Styles]\r\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\r\nStyle: Default,Arial,60,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,1,2,40,40,40,1\r\nStyle: Haut,Arial,50,&H0000FFFF,&H000000FF,&H00000000,&H80000000,1,0,0,0,100,100,0,0,1,2,1,8,40,40,40,1\r\n\r\n[Events]\r\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\r\nComment: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,note de l'auteur\r\nDialogue: 0,0:00:01.00,0:00:03.50,Default,Marie,0,0,0,,Bonjour, toi.\\NDeuxième ligne\r\nDialogue: 1,0:00:04.00,0:00:05.25,Haut,,10,10,80,,{\\i1}Au-dessus{\\i0}\r\n\r\n[Fonts]\r\n";

    #[test]
    fn reads_ass_dialogue_lines() {
        let cues = parse_ass(ASS.trim_start_matches('\u{feff}'));
        assert_eq!(cues.len(), 2, "le commentaire n'est pas un sous-titre");
        assert_eq!((cues[0].start, cues[0].end), (1.0, 3.5));
        assert_eq!(cues[0].text, "Bonjour, toi.\nDeuxième ligne");
        let f = cues[1].ass.as_ref().unwrap();
        assert_eq!(f["style"], "Haut");
        assert_eq!(f["layer"], "1");
        assert_eq!(f["marginv"], "80");
        assert_eq!(cues[1].text, "{\\i1}Au-dessus{\\i0}");
        assert_eq!(cues[1].end, 5.25);
        // Balises non comptées dans les contrôles.
        assert!((cps(&cues[1]) - 9.0 / 1.25).abs() < 1e-9);
    }

    #[test]
    fn writes_ass_keeping_everything_else() {
        let base = ASS.trim_start_matches('\u{feff}');
        let mut cues = parse_ass(base);
        cues[0].text = "Bonjour, vous.\nLigne deux".into();
        cues[0].end = 3.0;
        // Sous-titre ajouté dans l'éditeur (sans champs ASS).
        cues.push(Cue {
            start: 6.0,
            end: 7.5,
            text: "Nouveau".into(),
            ass: None,
        });
        let out = to_ass(&cues, Some(base));
        // En-tête, styles, commentaire et section [Fonts] recopiés.
        for keep in [
            "; Script de test",
            "Style: Haut,Arial,50",
            "Comment: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,note de l'auteur",
            "[Fonts]",
        ] {
            assert!(out.contains(keep), "{keep} manquant :\n{out}");
        }
        assert!(out.contains(
            "Dialogue: 0,0:00:01.00,0:00:03.00,Default,Marie,0,0,0,,Bonjour, vous.\\NLigne deux\n"
        ));
        assert!(out
            .contains("Dialogue: 1,0:00:04.00,0:00:05.25,Haut,,10,10,80,,{\\i1}Au-dessus{\\i0}\n"));
        // Le nouveau prend le style du premier, sans son nom.
        assert!(out.contains("Dialogue: 0,0:00:06.00,0:00:07.50,Default,,0,0,0,,Nouveau\n"));
        // Les dialogues restent dans [Events], avant [Fonts].
        assert!(out.find("Nouveau").unwrap() < out.find("[Fonts]").unwrap());
        assert_eq!(parse_ass(&out).len(), 3);
        assert_eq!(parse_ass(&out)[0].text, "Bonjour, vous.\nLigne deux");
    }

    #[test]
    fn ass_from_scratch_and_with_ffmpeg() {
        let cues = parse(SRT.trim_start_matches('\u{feff}'));
        let out = to_ass(&cues, None);
        assert!(out.starts_with("[Script Info]\n") && out.contains("Style: Default,Inter,64"));
        assert!(out.contains("Dialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Bonjour.\n"));
        assert_eq!(parse_ass(&out).len(), 2);
        assert_eq!(format_ass_time(3723.456), "1:02:03.46");
        // Fichiers enregistrés (modèle ASS, puis de zéro), relus par FFmpeg.
        let dir = tempfile::tempdir().unwrap();
        let template = dir.path().join("orig.ass");
        std::fs::write(&template, ASS).unwrap();
        let edited = dir.path().join("edit.ass");
        let cues = load(&template).unwrap();
        save(&edited, &cues, Some(&template)).unwrap();
        assert_eq!(load(&edited).unwrap(), cues);
        if crate::tools::locate("ffmpeg").is_some() {
            for f in [&edited, &template] {
                let o = crate::tools::command("ffmpeg")
                    .unwrap()
                    .args(["-v", "error", "-i"])
                    .arg(f)
                    .args(["-f", "srt", "-"])
                    .output()
                    .unwrap();
                assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
                let srt = String::from_utf8_lossy(&o.stdout);
                assert!(srt.contains("00:00:01,000 --> 00:00:03,500"), "{srt}");
            }
        }
    }

    #[test]
    fn times() {
        assert_eq!(parse_time("01:02:03,456"), Some(3723.456));
        assert_eq!(parse_time("02:03.5"), Some(123.5));
        assert_eq!(parse_time("1:75:00"), None);
        assert_eq!(format_time(3723.456, ','), "01:02:03,456");
    }
}
