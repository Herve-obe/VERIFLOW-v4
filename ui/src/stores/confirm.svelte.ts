// Fenêtres de confirmation propres à VERIFLOW (au lieu des boîtes du système),
// affichées par le composant ConfirmDialog monté à la racine.

export interface ConfirmOptions {
  title: string;
  /** Libellé du bouton de validation (par défaut « OK »). */
  ok?: string;
  /** Libellé du bouton d'abandon (par défaut « Annuler »). */
  cancel?: string;
  /** « danger » : action destructive, bouton de validation en rouge. */
  kind?: "warning" | "danger";
}

interface Pending extends ConfirmOptions {
  message: string;
  resolve: (ok: boolean) => void;
}

export const confirmQueue = $state({ items: [] as Pending[] });

/** Demande une confirmation ; vrai si l'utilisateur valide. */
export function ask(message: string, options: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    confirmQueue.items.push({ ...options, message, resolve });
  });
}

/** Réponse à la demande affichée (la première de la file). */
export function answer(ok: boolean) {
  const first = confirmQueue.items.shift();
  first?.resolve(ok);
}
