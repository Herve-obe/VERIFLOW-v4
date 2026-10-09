// Rapport SON. Modèle École : reproduction du rapport papier « Son 8 pistes »
// de l'école (A4 paysage, 8 pistes par feuillet ; au-delà, feuillets
// supplémentaires pour les pistes 9 à 16, etc.). Modèle Pro : colonnes
// étendues.
#import "common.typ": *
#show: setup

#set page(
  paper: "a4",
  flipped: true,
  margin: (x: 1cm, top: 0.9cm, bottom: 1cm),
  footer: footer,
)

#let head(sheet, count) = {
  let tracks = d.sheets.at(sheet - 1).columns.filter(c => c.key.starts-with("track_")).len()
  banner(tr("RAPPORT SON", "SOUND REPORT") + " " + str(tracks) + tr(" Pistes", " Tracks"), sheet, count)
  v(7pt)
  grid(
    columns: (1.5fr, 0.85fr, 1.35fr, 0.85fr, 1.2fr),
    column-gutter: 12pt,
    stack(
      spacing: 5.5pt,
      line-field("date"),
      line-field("title"),
      line-field("director"),
      line-field("sound_engineer"),
      line-field("boom"),
    ),
    stack(
      spacing: 6pt,
      group("IMAGE"),
      choices("film"),
      choices("fps"),
      other("film", "fps"),
    ),
    stack(
      spacing: 5.5pt,
      group(tr("ENREGISTREUR", "RECORDER")),
      line-field("recorder"),
      line-field("timecode"),
      line-field("sound_ref", unit: "dB FS"),
      line-field("recorder_other"),
    ),
    stack(
      spacing: 6pt,
      group(tr("NUMÉRISATION", "DIGITIZING")),
      choices("sample_rate"),
      choices("bits"),
      other("sample_rate", "bits"),
    ),
    remarks(2.6cm),
  )
  v(7pt)
}

#for (i, s) in d.sheets.enumerate() {
  if i > 0 { pagebreak() }
  head(i + 1, d.sheets.len())
  sheet-table(s, 0.68cm)
}
