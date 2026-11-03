// Niveau d'interface : DÉBUTANT (simplifié) ou EXPERT (complet, par défaut).
// Persistant (localStorage). Les réglages EXPERT ne sont jamais écrasés : le
// niveau DÉBUTANT ne fait que masquer ou surcharger au moment de l'action.
import type { Tab } from "./app.svelte";

export type Level = "beginner" | "expert";

const KEY = "veriflow.level";

function load(): Level {
  try {
    return localStorage.getItem(KEY) === "beginner" ? "beginner" : "expert";
  } catch {
    return "expert";
  }
}

export const level = $state({ value: load() });

export function setLevel(l: Level) {
  level.value = l;
  try {
    localStorage.setItem(KEY, l);
  } catch {
    /* stockage indisponible : le niveau vaut pour la session */
  }
}

export const isBeginner = () => level.value === "beginner";

/** Onglets absents de l'interface DÉBUTANT. */
export const HIDDEN_IN_BEGINNER: Tab[] = ["sync", "report"];

export const tabVisible = (tab: Tab) => !(isBeginner() && HIDDEN_IN_BEGINNER.includes(tab));

export const visibleTabs = (tabs: readonly Tab[]) => tabs.filter(tabVisible);
