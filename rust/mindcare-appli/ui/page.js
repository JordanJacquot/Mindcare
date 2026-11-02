// La page minimale de Mindcare dans Mindset (étape 4 du plan « Mindcare en Rust ») : le titre, la version, « Gérée
// par Mindset » ou non, les couleurs du thème. Elle dit à Rust quand elle devient visible ou cachée.
// Rien de ce qui arrive n'est exécuté ni mis en HTML : seulement du texte (textContent) et des couleurs vérifiées.
import { suivrePage, noterVisibilite } from './ipc.js';

const couleur = /^#[0-9A-Fa-f]{6}$/;

function afficher(etat) {
  if (!etat || typeof etat !== 'object') return;
  if (typeof etat.titre === 'string') document.getElementById('titre').textContent = etat.titre;
  if (typeof etat.version === 'string') document.getElementById('version').textContent = 'Version ' + etat.version;
  document.getElementById('geree').textContent = etat.geree === true ? 'Gérée par Mindset' : 'Pas gérée par Mindset';
  const couleurs = etat.couleurs && typeof etat.couleurs === 'object' ? etat.couleurs : {};
  for (const nom of ['p', 's', 't']) {
    if (typeof couleurs[nom] === 'string' && couleur.test(couleurs[nom])) {
      document.documentElement.style.setProperty('--couleur-' + nom, couleurs[nom]);
    }
  }
}

function signalerVisibilite() {
  noterVisibilite(document.visibilityState === 'visible').catch(() => {});
}

document.addEventListener('visibilitychange', signalerVisibilite);
suivrePage(afficher).catch(() => {});
signalerVisibilite();
