// Rapport SON. Modèle École : champs du rapport papier « Son 8 pistes » de
// l'école (A4 paysage, 8 pistes par feuillet ; au-delà, feuillets
// supplémentaires pour les pistes 9 à 16, etc.). Modèle Pro : colonnes
// étendues.
#import "common.typ": *
#show: setup

#let pro = d.template == "pro"
#set page(
  paper: "a4",
  flipped: true,
  margin: (x: 1.1cm, top: 1cm, bottom: 1.1cm),
  footer: footer,
)

#let head(sheet, count) = {
  let tracks = d.sheets.at(sheet - 1).columns.filter(c => c.key.starts-with("track_")).len()
  banner(tr("RAPPORT SON", "SOUND REPORT") + " · " + str(tracks) + tr(" PISTES", " TRACKS"), sheet, count)
  v(7pt)
  grid(
    columns: (1.25fr, 1.2fr, 1.05fr, 1fr),
    rows: (3.3cm,),
    column-gutter: 7pt,
    card(height: 100%, tr("Production", "Production"), stack(
      spacing: 5pt,
      row("date"),
      row("title"),
      row("director"),
      row("sound_engineer"),
      row("boom"),
    )),
    card(height: 100%, tr("Enregistreur", "Recorder"), stack(
      spacing: 5pt,
      row("recorder"),
      row("timecode"),
      row("sound_ref", unit: "dB FS"),
      row("recorder_other"),
    )),
    card(height: 100%, tr("Image et numérisation", "Picture and digitizing"), stack(
      spacing: 5pt,
      choices("film", label: tr("Pellicule", "Film")),
      choices("fps", label: tr("Cadence", "Frame rate")),
      choices("sample_rate", label: tr("Fréquence", "Sample rate")),
      choices("bits", label: tr("Résolution", "Bit depth")),
    )),
    remarks(100%),
  )
  v(7pt)
}

#for (i, s) in d.sheets.enumerate() {
  if i > 0 { pagebreak() }
  head(i + 1, d.sheets.len())
  if pro {
    block(width: 100%, stroke: 0.6pt + faint, radius: 4pt, clip: true, sheet-table(s, fill-page: false))
  } else {
    framed(sheet-table(s))
  }
}
