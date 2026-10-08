// Compatibilité avec les moteurs web anciens (WebKit de macOS 10.15 Catalina,
// niveau Safari 13), appliquée au CSS compilé de l'interface.
//
// 1. `:where(.svelte-xxx)` : Svelte 5 en génère pour ses styles ; un moteur
//    qui ne connaît pas `:where` ignore la règle entière. La classe seule
//    sélectionne les mêmes éléments (spécificité un peu plus forte, sans effet
//    ici puisque chaque composant n'en utilise qu'une).
// 2. `gap` dans un conteneur flex (Safari 14.1 et plus) : pour chaque règle
//    flex avec `gap`, une règle de remplacement en marges est ajoutée. Elle ne
//    s'applique que si boot.js a constaté l'absence de prise en charge (classe
//    `vf-nogap` sur <html>). Marges à droite (ligne) ou en bas (colonne), pour
//    ne pas écraser les `margin-left: auto` qui poussent un élément à droite.
import type { Plugin, Rule } from "postcss";

const WHERE = /:where\((\.[A-Za-z0-9_-]+)\)/g;

function gaps(rule: Rule): { row?: string; column?: string } {
  const out: { row?: string; column?: string } = {};
  rule.walkDecls((d) => {
    if (d.prop === "gap") {
      const [row, column = row] = d.value.trim().split(/\s+/);
      out.row = row;
      out.column = column;
    } else if (d.prop === "row-gap") out.row = d.value.trim();
    else if (d.prop === "column-gap") out.column = d.value.trim();
  });
  return out;
}

function decl(rule: Rule, prop: string): string | undefined {
  let v: string | undefined;
  rule.walkDecls(prop, (d) => {
    v = d.value.trim();
  });
  return v;
}

export default function oldWebkit(): Plugin {
  return {
    postcssPlugin: "veriflow-old-webkit",
    Rule(rule) {
      if (rule.selector.includes(":where(")) {
        rule.selector = rule.selector.replace(WHERE, "$1");
      }
    },
    OnceExit(root) {
      const extra: [Rule, Rule][] = [];
      // Seules les règles des composants compilés (classe svelte-xxx) sont
      // traitées : le CSS global n'a pas de conteneur flex avec gap, et avant
      // compilation (vérification svelte-check) Svelte croirait inutilisées
      // les règles ajoutées.
      root.walkRules((rule) => {
        if (!rule.selector.includes("svelte-")) return;
        const display = decl(rule, "display");
        if (display !== "flex" && display !== "inline-flex") return;
        const { row, column } = gaps(rule);
        if (!row && !column) return;
        const vertical = (decl(rule, "flex-direction") ?? "row").startsWith("column");
        const wraps = (decl(rule, "flex-wrap") ?? "nowrap") !== "nowrap";
        const main = vertical ? row : column;
        const cross = vertical ? column : row;
        const selectors = rule.selectors.map((s) => `html.vf-nogap ${s}`);
        if (main && main !== "0" && main !== "0px") {
          const r = rule.clone({ selector: selectors.map((s) => `${s} > *:not(:last-child)`).join(", ") });
          r.removeAll();
          r.append({ prop: vertical ? "margin-bottom" : "margin-right", value: main });
          extra.push([rule, r]);
        }
        if (wraps && cross && cross !== "0" && cross !== "0px") {
          const r = rule.clone({ selector: selectors.map((s) => `${s} > *`).join(", ") });
          r.removeAll();
          r.append({ prop: vertical ? "margin-right" : "margin-bottom", value: cross });
          extra.push([rule, r]);
        }
      });
      // Insérées juste après la règle d'origine, dans le même contexte (@media...).
      for (const [orig, r] of extra.reverse()) orig.after(r);
    },
  };
}
oldWebkit.postcss = true;
