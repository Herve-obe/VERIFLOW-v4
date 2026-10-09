//! Exports tableur et web d'un rapport : CSV (Excel français), XLSX, HTML.
//! Toutes les pistes et toutes les lignes, sans découpage en feuillets.

use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook};

use super::pdf::display_date;
use super::{columns, header_fields, Column, Lang, Report, ReportKind};
use crate::{Error, Result};

fn all_columns(report: &Report, lang: Lang) -> Vec<Column> {
    let tracks: Vec<usize> = match report.kind {
        ReportKind::Image => Vec::new(),
        ReportKind::Sound => (1..=report.tracks.max(1) as usize).collect(),
    };
    let mut cols = columns(report.kind, report.template, &tracks, lang);
    // Tableurs : une colonne « Cerclée » après la prise, pour filtrer les
    // bonnes prises (sur le PDF, le numéro de prise est entouré).
    let at = cols
        .iter()
        .position(|c| c.key == "take")
        .map_or(cols.len(), |i| i + 1);
    let label = if lang == Lang::Fr {
        "Cerclée"
    } else {
        "Circled"
    };
    cols.insert(
        at,
        Column {
            key: "circled".into(),
            label: label.into(),
            width: 0.8,
        },
    );
    cols
}

/// Titre du rapport (« Rapport Image N° 3 »).
pub fn title(report: &Report, lang: Lang) -> String {
    let base = match (report.kind, lang) {
        (ReportKind::Image, Lang::Fr) => "Rapport Image",
        (ReportKind::Image, Lang::En) => "Camera report",
        (ReportKind::Sound, Lang::Fr) => "Rapport Son",
        (ReportKind::Sound, Lang::En) => "Sound report",
    };
    if report.number > 0 {
        let n = if lang == Lang::Fr { "N°" } else { "#" };
        format!("{base} {n} {}", report.number)
    } else {
        base.into()
    }
}

/// En-tête : (libellé, valeur) des champs remplis, dans l'ordre du rapport.
fn header_lines(report: &Report, lang: Lang) -> Vec<(String, String)> {
    header_fields(report.kind)
        .iter()
        .filter_map(|f| {
            let v = report.header(f.key).trim();
            if v.is_empty() {
                return None;
            }
            let label = if lang == Lang::Fr {
                f.label_fr
            } else {
                f.label_en
            };
            let value = if f.key == "date" {
                display_date(v)
            } else if f.unit.is_empty() {
                v.to_string()
            } else {
                format!("{v} {}", f.unit)
            };
            Some((label.to_string(), value))
        })
        .collect()
}

fn csv_cell(s: &str) -> String {
    if s.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// CSV séparé par des points-virgules, avec BOM : s'ouvre directement dans
/// Excel en français. En-tête du rapport en premières lignes.
pub fn to_csv(report: &Report, lang: Lang) -> String {
    let mut out = String::from("\u{feff}");
    out += &csv_cell(&title(report, lang));
    out += "\n";
    for (k, v) in header_lines(report, lang) {
        out += &format!("{};{}\n", csv_cell(&k), csv_cell(&v));
    }
    out += "\n";
    let cols = all_columns(report, lang);
    let line = |cells: Vec<String>| {
        cells
            .iter()
            .map(|c| csv_cell(c))
            .collect::<Vec<_>>()
            .join(";")
    };
    out += &line(cols.iter().map(|c| c.label.clone()).collect());
    out += "\n";
    for r in &report.rows {
        out += &line(cols.iter().map(|c| r.get(&c.key).to_string()).collect());
        out += "\n";
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Page HTML autonome, imprimable.
pub fn to_html(report: &Report, lang: Lang) -> String {
    let t = esc(&title(report, lang));
    let mut h = format!(
        "<!doctype html><html lang=\"{}\"><head><meta charset=\"utf-8\"><title>{t}</title><style>\
body{{font:13px/1.45 system-ui,Segoe UI,Helvetica,sans-serif;color:#111;margin:28px}}h1{{font-size:20px;margin:0 0 10px}}\
.info td{{padding:2px 14px 2px 0}}.info td:first-child{{color:#555}}\
table.rows{{border-collapse:collapse;width:100%;margin-top:16px}}.rows th,.rows td{{border:1px solid #bbb;padding:4px 6px;text-align:left;vertical-align:top}}\
.rows th{{background:#eee}}.m{{font-family:ui-monospace,Consolas,monospace;white-space:nowrap}}\
@media print{{body{{margin:10mm}}}}</style></head><body><h1>{t}</h1><table class=\"info\">",
        if lang == Lang::Fr { "fr" } else { "en" }
    );
    for (k, v) in header_lines(report, lang) {
        h += &format!("<tr><td>{}</td><td>{}</td></tr>", esc(&k), esc(&v));
    }
    h += "</table><table class=\"rows\"><thead><tr>";
    let cols = all_columns(report, lang);
    for c in &cols {
        h += &format!("<th>{}</th>", esc(&c.label));
    }
    h += "</tr></thead><tbody>";
    for r in &report.rows {
        h += "<tr>";
        for c in &cols {
            let mono = matches!(c.key.as_str(), "tc_in" | "tc_out" | "duration" | "sound_tc");
            h += &format!(
                "<td{}>{}</td>",
                if mono { " class=\"m\"" } else { "" },
                esc(r.get(&c.key))
            );
        }
        h += "</tr>";
    }
    h += &format!(
        "</tbody></table><p style=\"color:#777;font-size:11px;margin-top:14px\">VERIFLOW {}</p></body></html>",
        crate::VERSION
    );
    h
}

/// Classeur Excel : une feuille, en-tête puis tableau.
pub fn to_xlsx(report: &Report, lang: Lang) -> Result<Vec<u8>> {
    let err = |e: rust_xlsxwriter::XlsxError| Error::Report(format!("XLSX : {e}"));
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name(match report.kind {
        ReportKind::Image => "Image",
        ReportKind::Sound => {
            if lang == Lang::Fr {
                "Son"
            } else {
                "Sound"
            }
        }
    })
    .map_err(err)?;
    let bold = Format::new().set_bold();
    let titlef = Format::new().set_bold().set_font_size(14);
    let muted = Format::new().set_font_color("#555555");
    let head = Format::new()
        .set_bold()
        .set_background_color("#E8E8E8")
        .set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Center);
    let cell = Format::new().set_border(FormatBorder::Thin).set_text_wrap();
    ws.write_string_with_format(0, 0, title(report, lang), &titlef)
        .map_err(err)?;
    let mut row = 2u32;
    for (k, v) in header_lines(report, lang) {
        ws.write_string_with_format(row, 0, k, &muted)
            .map_err(err)?;
        ws.write_string_with_format(row, 1, v, &bold).map_err(err)?;
        row += 1;
    }
    row += 1;
    let cols = all_columns(report, lang);
    for (i, c) in cols.iter().enumerate() {
        ws.write_string_with_format(row, i as u16, &c.label, &head)
            .map_err(err)?;
        ws.set_column_width(i as u16, (c.width as f64 * 11.0).max(8.0))
            .map_err(err)?;
    }
    ws.set_freeze_panes(row + 1, 0).map_err(err)?;
    for r in &report.rows {
        row += 1;
        for (i, c) in cols.iter().enumerate() {
            ws.write_string_with_format(row, i as u16, r.get(&c.key), &cell)
                .map_err(err)?;
        }
    }
    wb.save_to_buffer().map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{Row, Template};

    fn report() -> Report {
        let mut r = Report::new(ReportKind::Sound, Template::School);
        r.number = 2;
        r.tracks = 10;
        r.header.insert("title".into(), "Film; \"test\"".into());
        r.header.insert("sample_rate".into(), "48".into());
        let mut row = Row::default();
        row.fields.insert("id".into(), "T001".into());
        row.fields.insert("track_10".into(), "HF2".into());
        r.rows.push(row);
        r
    }

    #[test]
    fn csv_has_header_and_all_tracks() {
        let csv = to_csv(&report(), Lang::Fr);
        assert!(csv.starts_with("\u{feff}Rapport Son N° 2\n"));
        assert!(csv.contains("Titre du film;\"Film; \"\"test\"\"\"\n"));
        assert!(csv.contains("Numérisation;48 kHz\n"));
        assert!(csv.contains(";Piste 10;Observations\n"));
        assert!(csv.contains("Prise;Cerclée;Piste 1"));
        assert!(csv.trim_end().ends_with("T001;;;;;;;;;;;;;HF2;"));
    }

    #[test]
    fn html_escapes_text() {
        let html = to_html(&report(), Lang::Fr);
        assert!(html.contains("Film; &quot;test&quot;"));
        assert!(html.contains("<th>Piste 10</th>"));
    }

    #[test]
    fn xlsx_is_a_zip_file() {
        let x = to_xlsx(&report(), Lang::Fr).unwrap();
        assert!(x.starts_with(b"PK"));
    }
}
