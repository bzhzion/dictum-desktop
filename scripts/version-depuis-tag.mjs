// Ecrit la version du tag dans `src-tauri/tauri.conf.json`, juste avant de construire.
//
// ⚠️ Cette valeur n'est JAMAIS commitee : la convention du parc veut que la version vive dans les
// tags git et que la CI patche le manifeste a la volee. `tauri.conf.json` reste donc fige a
// `0.0.0` dans le depot.
//
// ⛔ Et ce n'est pas un detail cosmetique. Sans ce patch, l'installateur s'appelle
// `Oyant_0.0.0_x64-setup.exe`, « Programmes et fonctionnalites » affiche 0.0.0, et surtout
// **Windows refuse de mettre a jour une installation par une version qui n'est pas superieure** :
// toutes les versions se valant, la mise a jour ne se ferait jamais. Le binaire, lui, connait sa
// vraie version par `build.rs`, ce qui aurait donne un logiciel qui s'annonce 1.2.0 dans une
// installation enregistree comme 0.0.0.
//
// Usage : node scripts/version-depuis-tag.mjs [version]
// Sans argument, la version est lue dans GITHUB_REF_NAME (le `v` initial est retire).

import { readFileSync, writeFileSync } from 'node:fs';

const MANIFESTE = new URL('../src-tauri/tauri.conf.json', import.meta.url);

function versionDemandee() {
  const brut = process.argv[2] ?? process.env.GITHUB_REF_NAME ?? '';
  return brut.trim().replace(/^v/, '');
}

const version = versionDemandee();

// Refuse plutot que d'ecrire n'importe quoi : sur un declenchement manuel, GITHUB_REF_NAME vaut
// un nom de branche, qui produirait un manifeste invalide et un message d'erreur de Tauri sans
// rapport avec sa cause.
if (!/^\d+\.\d+\.\d+/.test(version)) {
  console.log(`Version « ${version || '(vide)'} » non conforme a X.Y.Z : manifeste laisse tel quel.`);
  process.exit(0);
}

const manifeste = JSON.parse(readFileSync(MANIFESTE, 'utf8'));
const ancienne = manifeste.version;
manifeste.version = version;
writeFileSync(MANIFESTE, `${JSON.stringify(manifeste, null, 2)}\n`, 'utf8');

console.log(`tauri.conf.json : version ${ancienne} -> ${version}`);
