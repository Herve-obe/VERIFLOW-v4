//! Sous-titres : lecture et écriture SRT et WebVTT, contrôles de lisibilité.
//!
//! Les temps sont en secondes depuis le début du média. Les contrôles suivent
//! les usages courants du sous-titrage (42 caractères par ligne, 2 lignes,
//! 17 caractères par seconde, 5/6 s à 7 s par sous-titre, 2 images d'écart).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Lit un fichier .srt ou .vtt.
pub fn load(path: &Path) -> Result<Vec<Cue>> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_start_matches('\u{feff}');
    match ext(path).as_str() {
        "srt" | "vtt" => Ok(parse(text)),
        other => Err(Error::Unsupported(format!(
            "sous-titres .{other} : l'éditeur lit les fichiers SRT et WebVTT"
        ))),
    }
}

/// Enregistre en SRT ou WebVTT selon l'extension.
pub fn save(path: &Path, cues: &[Cue]) -> Result<()> {
    let content = match ext(path).as_str() {
        "vtt" => to_vtt(cues),
        "srt" => to_srt(cues),
        other => {
            return Err(Error::Unsupported(format!(
                "sous-titres .{other} : enregistrer en .srt ou .vtt"
            )))
        }
    };
    std::fs::write(path, content)?;
    Ok(())
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
            },
            Cue {
                start: 0.52,
                end: 1.52,
                text: "Une ligne vraiment beaucoup trop longue pour un sous-titre".into(),
            },
            Cue {
                start: 1.4,
                end: 9.0,
                text: "a\nb\nc".into(),
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
        };
        assert_eq!(cps(&c), 3.0);
    }

    #[test]
    fn times() {
        assert_eq!(parse_time("01:02:03,456"), Some(3723.456));
        assert_eq!(parse_time("02:03.5"), Some(123.5));
        assert_eq!(parse_time("1:75:00"), None);
        assert_eq!(format_time(3723.456, ','), "01:02:03,456");
    }
}
