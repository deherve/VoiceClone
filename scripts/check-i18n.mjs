// Verifie les traductions de l'interface : memes cles en francais et en anglais, aucune cle manquante utilisee par
// index.html ou main.js, aucune variable {x} presente dans une langue et pas dans l'autre.
//   node scripts/check-i18n.mjs
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const ui = new URL('../crates/app/ui/', import.meta.url);
const read = (f) => readFileSync(new URL(f, ui), 'utf8');

// i18n.js declare `const I18N`, pas une propriete globale : on l'evalue puis on la relit.
const ctx = { navigator: { language: 'fr' }, localStorage: { getItem: () => null, setItem() {} }, document: {} };
vm.createContext(ctx);
vm.runInContext(read('i18n.js') + '\nglobalThis.__I18N = I18N;', ctx);
const { fr, en } = ctx.__I18N;

const problems = [];
const frKeys = new Set(Object.keys(fr)), enKeys = new Set(Object.keys(en));
for (const k of frKeys) if (!enKeys.has(k)) problems.push(`manque en anglais : ${k}`);
for (const k of enKeys) if (!frKeys.has(k)) problems.push(`manque en français : ${k}`);

const vars = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',');
for (const k of frKeys) if (enKeys.has(k) && vars(fr[k]) !== vars(en[k])) problems.push(`variables différentes pour ${k} : {${vars(fr[k])}} / {${vars(en[k])}}`);
// Mots identiques dans les deux langues par nature (noms propres, « audio », « Original »...) : pas des oublis.
const SAME = new Set(['nav.pages', 'nav.studio', 'lang.ui.fr', 'lang.ui.en', 'theme.nocturne', 'theme.atelier', 'theme.console', 'theme.conversation',
  'gen.original', 'gen.multiple', 'dur.sec', 'dur.min', 'dur.hour', 'pv.original', 'ask.ok', 'tab.interface', 'tab.audios', 'gen.count_one', 'gen.count_other', 'comp.mp3', 'readme.french', 'readme.english']);
for (const [k, v] of Object.entries(en)) {
  if (v.trim() === '') problems.push(`texte anglais vide : ${k}`);
  else if (fr[k] === en[k] && !SAME.has(k)) problems.push(`anglais identique au français (non traduit ?) : ${k}`);
}

// Cles utilisees : data-i18n="..." / data-i18n-attr="a:cle;b:cle" dans le HTML, t('...') et tp('...') dans main.js.
const used = new Map();
const use = (k, where) => used.set(k, [...(used.get(k) ?? []), where]);
const html = read('index.html'), js = read('main.js');
for (const m of html.matchAll(/data-i18n="([^"]+)"/g)) use(m[1], 'index.html');
for (const m of html.matchAll(/data-i18n-attr="([^"]+)"/g)) for (const p of m[1].split(';')) use(p.split(':')[1], 'index.html');
for (const m of js.matchAll(/\bt\(\s*'([^']+)'/g)) use(m[1], 'main.js');
for (const m of js.matchAll(/\btp\(\s*'([^']+)'/g)) { use(`${m[1]}_one`, 'main.js'); use(`${m[1]}_other`, 'main.js'); }
// Cles construites : t(`lang.${code}`), t(`theme.${id}`), t(`tri.${value}`), t(`comp.${id}`), stateKey de main.js.
for (const code of ['french', 'english', 'german', 'spanish', 'italian', 'portuguese', 'japanese', 'korean', 'chinese', 'russian']) use(`lang.${code}`, 'main.js (langues)');
for (const id of ['nocturne', 'atelier', 'console', 'conversation']) { use(`theme.${id}`, 'main.js (themes)'); use(`theme.${id}_desc`, 'main.js (themes)'); }
for (const v of ['auto', 'on', 'off']) use(`tri.${v}`, 'main.js (gpu)');
for (const c of ['fr', 'en']) use(`lang.ui.${c}`, 'main.js (langue)');
// t(cond ? 'a' : 'b', ...) : les deux cles sont utilisees.
for (const m of js.matchAll(/\bt\([^'`]*\?\s*'([^']+)'\s*:\s*'([^']+)'/g)) { use(m[1], 'main.js'); use(m[2], 'main.js'); }
for (const id of ['mp3', 'vulkan', 'cuda']) use(`comp.${id}`, 'main.js (composants)');
for (const m of js.matchAll(/'(comp\.[a-z_0-9]+)'/g)) use(m[1], 'main.js');
for (const m of js.matchAll(/'(models\.[a-z_]+|upd\.[a-z_]+)'/g)) use(m[1], 'main.js');
for (const [k, where] of used) if (!frKeys.has(k)) problems.push(`clé utilisée mais absente du dictionnaire : ${k} (${[...new Set(where)].join(', ')})`);
const unused = [...frKeys].filter((k) => !used.has(k));
// Une cle jamais utilisee est du code mort : signale sans bloquer.
if (unused.length) console.log(`clés inutilisées (${unused.length}) : ${unused.join(', ')}`);

if (problems.length) {
  console.error(problems.join('\n'));
  process.exit(1);
}
console.log(`i18n OK : ${frKeys.size} clés en français et en anglais, ${used.size} clés utilisées.`);
