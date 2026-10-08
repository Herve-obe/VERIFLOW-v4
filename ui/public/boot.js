/* Diagnostic de démarrage, volontairement en JavaScript ancien (ES5) pour
   fonctionner même sur un moteur web trop vieux pour l'interface : si
   l'interface ne s'affiche pas, la cause est écrite dans la fenêtre au lieu
   d'un écran vide. Couleurs en dur : les jetons du thème ne sont peut-être
   pas chargés à ce stade. */
(function () {
  // Espacement « gap » des conteneurs flex absent (WebKit antérieur à
  // Safari 14.1, macOS Catalina) : la classe vf-nogap active les marges de
  // remplacement générées à la compilation (ui/old-webkit.postcss.ts).
  try {
    var probe = document.createElement("div");
    probe.style.display = "flex";
    probe.style.flexDirection = "column";
    probe.style.rowGap = "1px";
    probe.style.position = "absolute";
    probe.appendChild(document.createElement("div"));
    probe.appendChild(document.createElement("div"));
    document.body.appendChild(probe);
    if (probe.scrollHeight !== 1) document.documentElement.className += " vf-nogap";
    document.body.removeChild(probe);
  } catch (e) {
    /* détection impossible : on garde le CSS standard */
  }

  var errors = [];

  function show(title) {
    var app = document.getElementById("app");
    if (!app || app.childNodes.length > 0) return;
    var box = document.createElement("pre");
    box.setAttribute(
      "style",
      "margin:24px;padding:16px;white-space:pre-wrap;font:13px/1.5 Menlo,Consolas,monospace;" +
        "color:#e6e8eb;background:#171a20;border:1px solid #ff4d4f;border-radius:6px"
    );
    box.textContent =
      title +
      "\n\n" +
      (errors.length ? errors.join("\n") : "Aucune erreur signalée.") +
      "\n\nMoteur web : " +
      navigator.userAgent +
      "\n\nMerci de transmettre une capture de ce message.";
    app.appendChild(box);
  }

  window.addEventListener("error", function (e) {
    errors.push((e.message || "Erreur") + (e.filename ? " (" + e.filename + ":" + e.lineno + ")" : ""));
  });
  window.addEventListener("unhandledrejection", function (e) {
    errors.push("Promesse rejetée : " + (e.reason && e.reason.message ? e.reason.message : String(e.reason)));
  });
  window.addEventListener("load", function () {
    setTimeout(function () {
      show("VERIFLOW : l'interface n'a pas pu démarrer.");
    }, 4000);
  });
})();
