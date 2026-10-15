// Point d'entrée de l'interface.
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "./theme/tokens.css";
import "./theme/base.css";
import { mount } from "svelte";

// La fenêtre de sortie moniteur charge la même page avec « ?monitor=1 ».
// Chaque fenêtre ne charge que son propre composant (et ses styles).
const monitor = new URLSearchParams(location.search).has("monitor");
const target = document.getElementById("app")!;
// Sans await au niveau du module : non pris en charge par le moteur web de
// macOS Catalina (cible safari13).
if (monitor) {
  import("./Monitor.svelte").then(({ default: Monitor }) => mount(Monitor, { target }));
} else {
  import("./App.svelte").then(({ default: App }) => mount(App, { target }));
}
