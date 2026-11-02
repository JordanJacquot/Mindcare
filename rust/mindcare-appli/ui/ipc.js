// Le seul fichier des pages qui parle à Rust (docs\regles-tauri.md du dépôt Mindset, règle 1) : withGlobalTauri est
// coupé, rien d'autre ne touche à l'objet interne de Tauri. Seules les deux commandes de l'appli sont appelées, et
// la fenêtre n'a la permission que de celles-là (capabilities\principale.json).

const tauri = window.__TAURI_INTERNALS__;

// Reçoit l'état de l'appli (titre, version, gérée par Mindset, couleurs du thème) tout de suite, puis à chaque
// changement : Rust l'envoie par un canal (tauri::ipc::Channel), sans que la page ait à redemander.
// Chaque envoi arrive avec son numéro ({ message, index }, puis { end, index } à la fermeture du canal) : les
// messages sont rendus dans l'ordre où Rust les a envoyés, comme le fait la bibliothèque JavaScript de Tauri.
export function suivrePage(recevoir) {
  let attendu = 0;
  const enAttente = new Map();
  const id = tauri.transformCallback((envoi) => {
    if (!envoi || typeof envoi.index !== 'number') return;
    enAttente.set(envoi.index, envoi);
    while (enAttente.has(attendu)) {
      const suivant = enAttente.get(attendu);
      enAttente.delete(attendu);
      attendu += 1;
      if (!suivant.end) recevoir(suivant.message);
    }
  });
  const canal = { toJSON: () => '__CHANNEL__:' + id };
  return tauri.invoke('suivre_page', { canal });
}

// Dit à Rust si la page est visible ou cachée (pour le journal)
export function noterVisibilite(visible) {
  return tauri.invoke('noter_visibilite', { visible: visible === true });
}
