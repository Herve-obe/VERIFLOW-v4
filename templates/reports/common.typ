// Éléments communs aux rapports VERIFLOW (image et son).
// Ce fichier est importé par image.typ et son.typ : modifier ici la police,
// les couleurs, les encadrés, les cases à cocher et le tableau de tous les
// rapports.

#let d = json("/data.json")
#let f = d.fields
#let fr = d.lang == "fr"
#let tr(a, b) = if fr { a } else { b }

// Couleurs (impression en niveaux de gris fidèle).
#let ink = luma(25)        // texte et bandeau
#let muted = luma(105)     // libellés
#let faint = luma(190)     // filets
#let paper = luma(246)     // fond des encadrés, lignes alternées
#let white = luma(255)

// Police et texte de tout le document : `#show: setup` dans chaque rapport.
#let setup(body) = {
  set text(font: "Inter", size: 8pt, fill: ink, lang: d.lang)
  set par(leading: 0.42em)
  body
}

// ---------- En-tête ----------

// Bandeau : école ou production, titre, et pastilles Rapport / Feuillet / Support.
#let pill(label, value) = box(
  inset: (x: 7pt, y: 4pt),
  radius: 3pt,
  fill: luma(55),
  {
    text(size: 6.5pt, fill: luma(185), upper(label))
    h(5pt)
    text(size: 9pt, weight: "bold", fill: white, if value == "" { "  " } else { value })
  },
)

#let banner(title, sheet, count) = block(
  width: 100%,
  fill: ink,
  radius: 4pt,
  inset: (x: 10pt, y: 8pt),
  grid(
    columns: (auto, 1fr, auto),
    column-gutter: 10pt,
    align: (left + horizon, left + horizon, right + horizon),
    if d.has_logo { box(fill: white, radius: 2pt, inset: 3pt, image(read("/logo.txt"), height: 0.9cm)) },
    {
      if d.organization != "" {
        text(size: 7pt, fill: luma(185), tracking: 0.4pt, upper(d.organization))
        linebreak()
      }
      text(size: 15pt, weight: "bold", fill: white, tracking: 0.6pt, title)
    },
    {
      pill(tr("Rapport N°", "Report #"), d.number)
      h(4pt)
      pill(tr("Feuillet", "Sheet"), str(sheet) + " / " + str(count))
      h(4pt)
      pill(tr("Support", "Backup"), f.backup.value)
    },
  ),
)

// Encadré titré.
#let card(title, body, height: auto) = block(
  width: 100%,
  height: height,
  fill: paper,
  radius: 4pt,
  inset: (x: 8pt, top: 6pt, bottom: 7pt),
  {
    text(size: 6.5pt, weight: "bold", fill: muted, tracking: 0.6pt, upper(title))
    v(5pt)
    body
  },
)

// Valeur saisie, ou filet discret pour l'écrire à la main.
#let value(v) = if v == none or v == "" {
  box(width: 1fr, height: 8pt, stroke: (bottom: 0.5pt + faint))
} else {
  text(size: 8.5pt, weight: "semibold", v)
}

// Ligne « libellé  valeur » d'un encadré.
#let row(key, unit: none, label: none) = {
  let x = f.at(key)
  grid(
    columns: (auto, 1fr),
    column-gutter: 6pt,
    align: (left + bottom, left + bottom),
    text(fill: muted, if label == none { x.label } else { label }),
    {
      value(x.value)
      if unit != none and x.value != "" { h(2pt); text(fill: muted, unit) }
    },
  )
}

// Case : contour, ou pleine (texte blanc) si cochée.
#let chip(label, checked) = box(
  inset: (x: 5pt, y: 2.5pt),
  radius: 2pt,
  stroke: 0.6pt + (if checked { ink } else { faint }),
  fill: if checked { ink } else { white },
  text(size: 7.5pt, weight: if checked { "bold" } else { "regular" }, fill: if checked { white } else { muted }, label),
)

// Champ à choix : cases du rapport papier, puis la valeur hors cases.
#let choices(key, label: none) = {
  let x = f.at(key)
  grid(
    columns: (auto, 1fr),
    column-gutter: 6pt,
    align: (left + horizon, left + horizon),
    text(fill: muted, if label == none { x.label } else { label }),
    {
      for o in x.options { chip(o.label, o.checked); h(3pt) }
      if x.other != "" { chip(x.other, true); h(3pt) } else { chip(tr("autre", "other"), false); h(3pt) }
      if x.unit != "" { text(fill: muted, x.unit) }
    },
  )
}

// Encadré Remarques.
#let remarks(height) = card(f.remarks.label, height: height, text(size: 8.5pt, f.remarks.value))

// ---------- Tableau ----------

// Tableau d'un feuillet. Modèle École : lignes de hauteur égale qui occupent
// toute la page (place pour écrire à la main). Modèle Pro : hauteur selon le
// texte. Prise cerclée : numéro de prise entouré, comme sur un rapport papier.
#let sheet-table(s, fill-page: true) = {
  let mono = ("tc_in", "tc_out", "duration", "sound_tc")
  let n = s.rows.len()
  let body-rows = if fill-page { (1fr,) * n } else { (auto,) * n }
  table(
    columns: s.columns.map(c => c.width * 1fr),
    rows: (auto,) + body-rows,
    stroke: (x, y) => (
      top: if y <= 1 { none } else { 0.4pt + faint },
      left: if x == 0 { none } else { 0.4pt + faint },
    ),
    fill: (x, y) => if y == 0 { ink } else if calc.even(y) { paper } else { none },
    inset: (x: 5pt, y: 4pt),
    align: left + horizon,
    table.header(
      ..s.columns.map(c => text(size: 6.5pt, weight: "bold", fill: white, tracking: 0.1pt, upper(c.label))),
    ),
    ..s.rows.enumerate().map(((r, cells)) => cells.enumerate().map(((i, v)) => {
      let key = s.columns.at(i).key
      if key == "take" and s.circled.at(r) and v != "" {
        box(stroke: 0.9pt + ink, radius: 50%, inset: (x: 4pt, y: 2pt), text(weight: "bold", v))
      } else if key == "circled" {
        if v != "" { box(width: 7pt, height: 7pt, radius: 50%, fill: ink) }
      } else if key in mono {
        text(font: "DejaVu Sans Mono", size: 7.5pt, v)
      } else { v }
    })).flatten(),
  )
}

// Cadre extérieur du tableau (le tableau lui-même n'a que des filets).
#let framed(body) = block(width: 100%, height: 1fr, stroke: 0.6pt + faint, radius: 4pt, clip: true, body)

#let footer = context [
  #set text(size: 6.5pt, fill: muted)
  #d.footer #h(1fr) #d.edited #h(1fr) #tr("Page", "Page") #counter(page).display() / #counter(page).final().first()
]
