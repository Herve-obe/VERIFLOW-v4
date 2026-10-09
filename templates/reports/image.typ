// Rapport IMAGE. Modèle École : reproduction du rapport papier de l'école
// (A4 portrait). Modèle Pro : même en-tête, tableau étendu (A4 paysage).
#import "common.typ": *
#show: setup

#let pro = d.template == "pro"
#set page(
  paper: "a4",
  flipped: pro,
  margin: (x: 1.1cm, top: 1cm, bottom: 1.1cm),
  footer: footer,
)

#let head(sheet, count) = {
  banner(tr("RAPPORT Image", "CAMERA REPORT"), sheet, count)
  v(8pt)
  grid(
    columns: (1.25fr, 1fr, 1fr),
    column-gutter: 14pt,
    // Production
    stack(
      spacing: 5.5pt,
      line-field("date"),
      line-field("title"),
      line-field("director"),
      line-field("dop"),
      line-field("operator"),
      block(height: 2pt),
      line-field("camera"),
      line-field("image_format"),
      line-field("sound_ref", unit: "dB FS"),
    ),
    // Image et média
    stack(
      spacing: 6pt,
      group("IMAGE"),
      choices("definition"),
      choices("fps"),
      other("definition", "fps"),
      block(height: 4pt),
      group("MEDIA"),
      choices("media"),
      other("media"),
    ),
    remarks(3.6cm),
  )
  v(8pt)
}

#for (i, s) in d.sheets.enumerate() {
  if i > 0 { pagebreak() }
  head(i + 1, d.sheets.len())
  sheet-table(s, if pro { 0.62cm } else { 0.78cm })
}
