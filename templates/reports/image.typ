// Rapport IMAGE : champs du rapport papier de l'école. A4 portrait avec les
// colonnes de base, paysage dès qu'une colonne est ajoutée.
#import "common.typ": *
#show: setup

#let wide = d.landscape
#set page(
  paper: "a4",
  flipped: wide,
  margin: (x: 1.2cm, top: 1.1cm, bottom: 1.2cm),
  footer: footer,
)

#let head(sheet, count) = {
  banner(tr("RAPPORT IMAGE", "CAMERA REPORT"), sheet, count)
  v(7pt)
  grid(
    columns: if wide { (1.1fr, 1fr, 1.15fr, 1fr) } else { (1fr, 1fr) },
    rows: if wide { (3.3cm,) } else { (3.3cm, 2.5cm) },
    column-gutter: 7pt,
    row-gutter: 7pt,
    card(height: 100%, tr("Production", "Production"), stack(
      spacing: 6pt,
      row("date"),
      row("title"),
      row("director"),
      row("dop"),
      row("operator"),
    )),
    card(height: 100%, tr("Caméra", "Camera"), stack(
      spacing: 6pt,
      row("camera"),
      row("image_format"),
      row("sound_ref", unit: "dB FS"),
    )),
    card(height: 100%, tr("Image et support", "Image and media"), stack(
      spacing: 6pt,
      choices("definition", label: tr("Définition", "Resolution")),
      choices("fps", label: tr("Cadence", "Frame rate")),
      choices("media", label: tr("Support", "Media")),
    )),
    remarks(100%),
  )
  v(7pt)
}

#for (i, s) in d.sheets.enumerate() {
  if i > 0 { pagebreak() }
  head(i + 1, d.sheets.len())
  framed(sheet-table(s))
}
