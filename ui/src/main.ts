// Point d'entrée de l'interface.
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "./theme/tokens.css";
import "./theme/base.css";
import { mount } from "svelte";

// La fenêtre de sortie moniteur charge la même page avec « ?monitor=1 ».
// Chaque fenêtre ne charge que son propre composant (et ses styles).
// Le lecteur rapide de MEDIA, de même, avec « ?quick=1 ».
const params = new URLSearchParams(location.search);
const monitor = params.has("monitor");
const quick = params.has("quick");
const target = document.getElementById("app")!;
// Sans await au niveau du module : non pris en charge par le moteur web de
// macOS Catalina (cible safari13).
if (monitor) {
  import("./Monitor.svelte").then(({ default: Monitor }) => mount(Monitor, { target }));
} else if (quick) {
  import("./QuickWindow.svelte").then(({ default: QuickWindow }) => mount(QuickWindow, { target }));
} else {
  // « Repartir à zéro à chaque ouverture » (Préférences) : avant que les vues
  // ne lisent leurs réglages.
  import("./lib/appdata")
    .then(({ applyStartupMode }) => applyStartupMode())
    .then(() => import("./App.svelte"))
    .then(({ default: App }) => mount(App, { target }));
}
