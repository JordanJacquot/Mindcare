// La page minimale de Mindcare dans Mindset (étape 4 du plan « Mindcare en Rust ») : le titre, la version, « Gérée
// par Mindset » ou non, les couleurs du thème. Elle dit à Rust quand elle devient visible ou cachée (noté au journal :
// cachée, WebView2 la met en pause).
// Rien de ce qui arrive n'est exécuté ni mis en HTML : seulement du texte (textContent) et des couleurs vérifiées.
import { suivrePage, noterVisibilite } from './ipc.js';

const couleur = /^#[0-9A-Fa-f]{6}$/;
const nomVariable = /^[A-Za-z0-9]{1,16}$/;

function afficher(etat) {
  if (!etat || typeof etat !== 'object') return;
  if (typeof etat.titre === 'string') document.getElementById('titre').textContent = etat.titre;
  if (typeof etat.version === 'string') document.getElementById('version').textContent = 'Version ' + etat.version;
  document.getElementById('geree').textContent = etat.geree === true ? 'Gérée par Mindset' : 'Pas gérée par Mindset';
  // Les variables CSS du thème (--P, --S, --T, --fond, --fond2), seulement des couleurs « #RRGGBB »
  const couleurs = etat.couleurs && typeof etat.couleurs === 'object' ? etat.couleurs : {};
  for (const nom of Object.keys(couleurs)) {
    if (nomVariable.test(nom) && typeof couleurs[nom] === 'string' && couleur.test(couleurs[nom])) {
      document.documentElement.style.setProperty('--' + nom, couleurs[nom]);
    }
  }
}

function signalerVisibilite() {
  noterVisibilite(document.visibilityState === 'visible').catch(() => {});
}

document.addEventListener('visibilitychange', signalerVisibilite);
suivrePage(afficher).catch(() => {});
signalerVisibilite();
