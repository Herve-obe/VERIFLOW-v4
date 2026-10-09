// Éléments communs aux rapports VERIFLOW (image et son).
// Ce fichier est importé par image.typ et son.typ : modifier ici la
// police, les couleurs et les cases à cocher de tous les rapports.

#let d = json("/data.json")
#let f = d.fields
#let fr = d.lang == "fr"
#let tr(a, b) = if fr { a } else { b }

#let ink = luma(20)
#let muted = luma(110)
#let rule = luma(150)
#let headfill = luma(232)

// Police et texte de tout le document : `#show: setup` dans chaque rapport.
#let setup(body) = {
  set text(font: "Inter", size: 8pt, fill: ink, lang: d.lang)
  set par(leading: 0.45em)
  body
}

// Case à cocher : carré vide, ou marqué d'une croix.
#let cb(checked) = box(baseline: 1pt, width: 7pt, height: 7pt, stroke: 0.6pt + ink, {
  if checked {
    place(line(start: (1pt, 1pt), end: (6pt, 6pt), stroke: 0.9pt + ink))
    place(line(start: (6pt, 1pt), end: (1pt, 6pt), stroke: 0.9pt + ink))
  }
})

// Zone à remplir : la valeur, ou une ligne pointillée pour l'écrire à la main.
#let blank(value, width: 1fr) = box(
  width: width,
  stroke: (bottom: (paint: rule, thickness: 0.5pt, dash: "dotted")),
  inset: (bottom: 2pt),
  outset: (bottom: 0pt),
  text(weight: "medium", if value == none or value == "" { h(0pt) } else { value }),
)

// Ligne « libellé : valeur ».
#let line-field(key, unit: none) = {
  let x = f.at(key)
  grid(
    columns: (auto, 1fr),
    column-gutter: 4pt,
    text(fill: muted, x.label + " :"),
    {
      blank(x.value)
      if unit != none { h(3pt); unit }
    },
  )
}

// Cases d'un champ à choix, avec son unité (« 24 ☐ 25 ☐ i/s »).
#let choices(key) = {
  let x = f.at(key)
  box({
    for o in x.options [#o.label#h(3pt)#cb(o.checked)#h(9pt)]
    if x.unit != "" { x.unit }
  })
}

// « autre : ... » pour les valeurs hors des cases de plusieurs champs.
#let other(..keys) = {
  let vals = keys.pos().map(k => f.at(k).other).filter(v => v != "")
  grid(
    columns: (auto, 1fr),
    column-gutter: 4pt,
    text(fill: muted, tr("autre :", "other:")),
    blank(vals.join(" / ")),
  )
}

// Titre de groupe (IMAGE, MEDIA, ENREGISTREUR...).
#let group(title) = text(weight: "bold", size: 8pt, tracking: 0.5pt, title)

// Cadre « Remarques ».
#let remarks(height) = rect(
  width: 100%,
  height: height,
  stroke: 0.6pt + rule,
  radius: 2pt,
  inset: 5pt,
  {
    group(upper(f.remarks.label))
    v(3pt)
    f.remarks.value
  },
)

// En-tête de feuillet : logo, titre, numéros.
#let banner(title, sheet, count) = {
  grid(
    columns: (auto, 1fr, auto),
    column-gutter: 10pt,
    align: (left + horizon, left + horizon, right + horizon),
    if d.has_logo { image(read("/logo.txt"), height: 1.1cm) },
    {
      if d.organization != "" { text(fill: muted, d.organization); linebreak() }
      text(size: 15pt, weight: "bold", title)
    },
    {
      set align(right)
      tr("Rapport N°", "Report #") + " "
      blank(d.number, width: 1.4cm)
      h(10pt)
      tr("Feuillet N°", "Sheet #") + " " + str(sheet) + " / " + str(count)
      linebreak()
      v(2pt)
      f.backup.label + " : "
      blank(f.backup.value, width: 2.2cm)
    },
  )
}

// Tableau d'un feuillet. Les timecodes sont en chasse fixe. Les lignes ont
// une hauteur minimale (place pour écrire à la main) et s'agrandissent si
// le texte passe sur plusieurs lignes.
#let sheet-table(s, row-height) = {
  let mono = ("tc_in", "tc_out", "duration", "sound_tc")
  // Gabarit invisible qui donne sa hauteur minimale à la ligne, centré sur
  // le texte pour ne pas le décaler.
  let strut = box(width: 0pt, height: row-height - 6pt, baseline: (row-height - 6pt) / 2 - 0.35em)
  table(
    columns: s.columns.map(c => c.width * 1fr),
    stroke: 0.5pt + rule,
    inset: (x: 4pt, y: 3pt),
    align: horizon,
    table.header(
      ..s.columns.map(c => table.cell(fill: headfill, text(weight: "bold", size: 7.5pt, c.label))),
    ),
    ..s.rows.map(r => r.enumerate().map(((i, v)) => {
      let key = s.columns.at(i).key
      let body = if key in mono { text(font: "DejaVu Sans Mono", size: 7pt, v) } else { v }
      if i == 0 { strut + body } else { body }
    })).flatten(),
  )
}

#let footer = context [
  #set text(size: 6.5pt, fill: muted)
  #d.footer #h(1fr) #counter(page).display() / #counter(page).final().first()
]
