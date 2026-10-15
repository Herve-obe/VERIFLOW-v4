// Point d'entrée de l'interface.
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "./theme/tokens.css";
import "./theme/base.css";
import { mount } from "svelte";
import App from "./App.svelte";
import Monitor from "./Monitor.svelte";

// La fenêtre de sortie moniteur charge la même page avec « ?monitor=1 ».
const monitor = new URLSearchParams(location.search).has("monitor");
const app = mount(monitor ? Monitor : App, { target: document.getElementById("app")! });

export default app;
