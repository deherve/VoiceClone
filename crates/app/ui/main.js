// Interface VoiceClone : tout le texte passe par textContent (jamais innerHTML).
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);

const LANGS = { french: 'Français', english: 'Anglais', german: 'Allemand', spanish: 'Espagnol', italian: 'Italien',
  portuguese: 'Portugais', japanese: 'Japonais', korean: 'Coréen', chinese: 'Chinois', russian: 'Russe' };
// Texte propose a la lecture au micro (profils en francais) : il sert ensuite de transcription.
const READ_ME = "Bonjour, et bienvenue. Aujourd'hui, je prends un moment pour enregistrer ma voix, calmement, sans me presser. "
  + "Est-ce que le ton est naturel ? Je crois que oui. Je parle comme d'habitude, avec des phrases courtes, puis d'autres un peu plus longues, "
  + "pour donner du rythme. Voilà, c'est terminé : merci de m'avoir écouté !";
const WARN_AFTER_SEC = 90; // mesure : a 3 min de reference la generation deraille, a 1 min 30 elle tient

let profiles = [], current = null, samples = [], busy = false, recorder = null, playingKey = null;

function el(tag, props = {}, ...kids) {
  const n = Object.assign(document.createElement(tag), props);
  n.append(...kids);
  return n;
}
const fmt = (s) => { s = Math.round(s); return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`; };
const say = (where, msg) => { where.textContent = msg; where.classList.remove('error'); };
const fail = (where, err) => { where.textContent = `Erreur : ${err?.message ?? err}`; where.classList.add('error'); };
const profile = () => profiles.find((p) => p.id === current);

// ---------------------------------------------------------------- lecture audio
const PLAY = 'M7 4.5v15l13-7.5z', PAUSE = 'M7 4h4v16H7zM13 4h4v16h-4z';
const initials = (name) => name.trim().split(/\s+/).map((w) => w[0]).join('').slice(0, 2).toUpperCase();

function markPlaying() {
  for (const b of document.querySelectorAll('button[data-key]')) {
    const on = b.dataset.key === playingKey;
    if (b.classList.contains('play-circle')) {
      b.replaceChildren(icon(on ? PAUSE : PLAY));
      b.setAttribute('aria-label', on ? 'Arrêter la lecture' : 'Écouter');
    } else {
      b.textContent = on ? 'Arrêter' : 'Écouter';
    }
    b.setAttribute('aria-pressed', on);
  }
  for (const box of document.querySelectorAll('.bars')) {
    const on = box.dataset.key === playingKey;
    box.classList.toggle('playing', on);
    if (!on) box.querySelectorAll('.played').forEach((i) => i.classList.remove('played'));
  }
}
// La forme d'onde se remplit au fil de la lecture.
$('player').ontimeupdate = () => {
  const p = $('player'), box = document.querySelector(`.bars[data-key="${playingKey}"]`);
  if (!box || !p.duration) return;
  const done = Math.floor((p.currentTime / p.duration) * box.children.length);
  [...box.children].forEach((bar, k) => bar.classList.toggle('played', k < done));
};
function playButton(kind, id, where) {
  const b = el('button', { type: 'button', className: kind === 'generation' ? 'play-circle' : 'ghost' });
  b.dataset.key = `${kind}:${id}`;
  b.onclick = async () => {
    const p = $('player');
    const stop = playingKey === b.dataset.key;
    p.pause();
    playingKey = null;
    if (!stop) {
      try {
        const bytes = await invoke('audio', { kind, id });
        if (p.src) URL.revokeObjectURL(p.src);
        p.src = URL.createObjectURL(new Blob([bytes], { type: 'audio/wav' }));
        playingKey = b.dataset.key;
        await p.play();
      } catch (e) { playingKey = null; fail(where, e); }
    }
    markPlaying();
  };
  return b;
}
// Un "pause" tardif d'une lecture precedente ne doit pas eteindre le bouton de la nouvelle.
$('player').onpause = $('player').onended = () => { if ($('player').paused) { playingKey = null; markPlaying(); } };

// ---------------------------------------------------------------- voix
async function loadProfiles() {
  profiles = await invoke('profiles');
  if (!profile()) current = profiles[0]?.id ?? null;
  $('profile-list').replaceChildren(...profiles.map((p) => {
    const pick = el('button', { type: 'button', className: 'profile', onclick: () => { current = p.id; loadProfiles().catch((e) => fail($('status'), e)); } },
      el('span', { className: 'avatar', textContent: initials(p.name) }),
      el('span', { className: 'who' },
        el('strong', { textContent: p.name }),
        el('span', { textContent: `${LANGS[p.language] ?? p.language} · ${p.samples} échantillon${p.samples > 1 ? 's' : ''}` })));
    pick.setAttribute('aria-pressed', p.id === current);
    const del = el('button', { type: 'button', className: 'ghost danger icon', title: 'Supprimer cette voix' });
    del.append(icon(...ICON_TRASH));
    del.setAttribute('aria-label', `Supprimer la voix ${p.name}`);
    del.onclick = async () => {
      if (!confirm(`Supprimer la voix « ${p.name} » et ses échantillons ? Cette action est définitive.`)) return;
      try { await invoke('delete_profile', { id: p.id }); await loadProfiles(); await loadGenerations(); } catch (e) { fail($('status'), e); }
    };
    return el('li', {}, pick, del);
  }));
  if (!profiles.length) $('new-profile-box').open = true;
  if (!profiles.length) $('profile-list').append(el('li', { className: 'empty', textContent: 'Aucune voix. Créez-en une ci-dessous.' }));
  await loadSamples();
  updateGo();
}

$('np-lang').append(...Object.entries(LANGS).map(([value, textContent]) => el('option', { value, textContent })));
$('new-profile').onsubmit = async (ev) => {
  ev.preventDefault();
  try {
    current = await invoke('create_profile', { name: $('np-name').value, language: $('np-lang').value });
    $('np-name').value = '';
    $('new-profile-box').open = false;
    await loadProfiles();
    say($('sample-status'), 'Voix créée. Ajoutez maintenant un échantillon.');
  } catch (e) { fail($('status'), e); }
};

// ---------------------------------------------------------------- echantillons
function sampleNotes() {
  const total = samples.reduce((t, s) => t + s.duration_sec, 0);
  const notes = [];
  if (!samples.length) notes.push('Ajoutez un échantillon pour pouvoir générer avec cette voix.');
  if (samples.some((s) => !s.ref_text.trim())) notes.push('Transcription manquante : le clonage se limite au timbre de la voix, il est moins fidèle.');
  if (total > WARN_AFTER_SEC) notes.push(`${fmt(total)} d'échantillons au total : au-delà de ${fmt(WARN_AFTER_SEC)} environ, la génération ralentit et peut dérailler.`);
  $('sample-notes').textContent = notes.join(' ');
}

async function loadSamples() {
  const p = profile();
  $('samples-panel').hidden = !p;
  if (!p) return;
  $('sp-name').textContent = p.name;
  samples = await invoke('samples', { profileId: p.id });
  const st = $('sample-status');
  $('sample-list').replaceChildren(...samples.map((s, i) => {
    const text = el('textarea', { value: s.ref_text, rows: 3, placeholder: 'Transcription exacte de cet échantillon' });
    text.setAttribute('aria-label', `Transcription de l'échantillon ${i + 1}`);
    text.onchange = async () => {
      try { await invoke('set_sample_text', { id: s.id, text: text.value }); s.ref_text = text.value; sampleNotes(); say(st, 'Transcription enregistrée.'); }
      catch (e) { fail(st, e); }
    };
    const del = el('button', { type: 'button', className: 'ghost danger', textContent: 'Supprimer' });
    del.setAttribute('aria-label', `Supprimer l'échantillon ${i + 1}`);
    del.onclick = async () => {
      if (!confirm(`Supprimer l'échantillon ${i + 1} ? Cette action est définitive.`)) return;
      try { await invoke('delete_sample', { id: s.id }); await loadProfiles(); } catch (e) { fail(st, e); }
    };
    return el('li', { className: 'sample' },
      el('div', { className: 'row' }, el('span', { className: 'grow', textContent: `Échantillon ${i + 1} · ${fmt(s.duration_sec)}` }), playButton('sample', s.id, st), del),
      text);
  }));
  markPlaying();
  sampleNotes();
}

// La webview decode n'importe quel format audio et reechantillonne en 24 kHz mono pour le moteur.
async function to24k(arrayBuffer) {
  const ctx = new AudioContext();
  const decoded = await ctx.decodeAudioData(arrayBuffer).finally(() => ctx.close());
  const off = new OfflineAudioContext(1, Math.ceil(decoded.duration * 24000), 24000);
  const src = off.createBufferSource();
  src.buffer = decoded;
  src.connect(off.destination);
  src.start();
  return (await off.startRendering()).getChannelData(0);
}

async function addSample(arrayBuffer, profileId, transcript) {
  const st = $('sample-status');
  say(st, "Conversion de l'audio…");
  const pcm = await to24k(arrayBuffer);
  const id = await invoke('add_sample', new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength), { headers: { 'x-profile': String(profileId) } });
  if (transcript) await invoke('set_sample_text', { id, text: transcript });
  await loadProfiles();
  say(st, transcript ? 'Échantillon ajouté. Corrigez la transcription si vous avez dit autre chose.' : 'Échantillon ajouté. Saisissez sa transcription exacte.');
}

$('rec').onclick = async () => {
  const st = $('sample-status'), btn = $('rec');
  if (recorder) { recorder.stop(); return; }
  const p = profile();
  let stream;
  try { stream = await navigator.mediaDevices.getUserMedia({ audio: true }); }
  catch (e) { fail(st, `micro inaccessible (${e.message})`); return; }
  const chunks = [], t0 = Date.now();
  const guided = p.language === 'french';
  const tick = () => { $('rec-label').textContent = `Arrêter (${fmt((Date.now() - t0) / 1000)})`; };
  const timer = setInterval(tick, 500);
  recorder = new MediaRecorder(stream);
  recorder.ondataavailable = (e) => chunks.push(e.data);
  recorder.onstop = async () => {
    stream.getTracks().forEach((t) => t.stop());
    clearInterval(timer);
    recorder = null;
    $('rec-label').textContent = 'Enregistrer';
    btn.classList.remove('recording');
    $('read-me').hidden = true;
    try { await addSample(await new Blob(chunks).arrayBuffer(), p.id, guided ? READ_ME : ''); } catch (e) { fail(st, e); }
  };
  recorder.start();
  btn.classList.add('recording');
  tick();
  $('read-me').textContent = READ_ME;
  $('read-me').hidden = !guided;
  say(st, guided ? 'Enregistrement en cours. Lisez le texte ci-dessus, ou dites autre chose et corrigez la transcription ensuite.' : 'Enregistrement en cours.');
};

$('import').onclick = () => $('file').click();
$('file').onchange = async (ev) => {
  const file = ev.target.files[0];
  ev.target.value = '';
  if (!file) return;
  try { await addSample(await file.arrayBuffer(), current, ''); }
  catch (e) { fail($('sample-status'), `fichier audio illisible (${e?.message ?? e})`); }
};

// ---------------------------------------------------------------- generation
function updateGo() {
  const p = profile(), n = $('text').value.trim().length;
  $('go').disabled = busy || !p || !p.samples || !n;
  $('clear').disabled = !$('text').value;
  $('count').replaceChildren(...(p
    ? [el('span', { className: 'avatar', textContent: initials(p.name) }), `Voix : ${p.name} · ${n} caractère${n > 1 ? 's' : ''}`]
    : ['Aucune voix sélectionnée']));
}

async function generate() {
  const p = profile(), text = $('text').value.trim(), st = $('status');
  if (busy || !p || !text) return;
  busy = true;
  updateGo();
  say(st, 'Génération en cours… Le premier appel charge le modèle, il est plus long.');
  try {
    const r = await invoke('speak', { text, profile: p.name });
    say(st, `${fmt(r.duration_sec)} d'audio générées en ${r.elapsed_sec} s.`);
  } catch (e) { fail(st, e); }
  busy = false;
  updateGo();
}
$('go').onclick = generate;
$('text').oninput = updateGo;
$('text').onkeydown = (ev) => { if (ev.ctrlKey && ev.key === 'Enter') generate(); };
// Efface par une commande d'edition : Ctrl+Z ramene le texte (une affectation de .value l'effacerait pour de bon).
$('clear').onclick = () => {
  const t = $('text');
  t.focus();
  t.select();
  if (!document.execCommand('delete')) t.value = '';
  updateGo();
};

// ---------------------------------------------------------------- historique
function icon(...paths) {
  const NS = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('aria-hidden', 'true');
  for (const d of paths) {
    const p = document.createElementNS(NS, 'path');
    p.setAttribute('d', d);
    svg.append(p);
  }
  return svg;
}
const ICON_COPY = ['M9 9h11a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H9a2 2 0 0 1-2-2v-9a2 2 0 0 1 2-2z', 'M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1'];
const ICON_CHECK = ['M20 6 9 17l-5-5'];
const ICON_TRASH = ['M3 6h18', 'M8 6V4h8v2', 'M19 6l-1 14H6L5 6', 'M10 11v6M14 11v6'];

async function copyText(text) {
  try { await navigator.clipboard.writeText(text); }
  catch {
    // Repli : la webview peut refuser l'API presse-papiers hors geste utilisateur.
    const t = el('textarea', { value: text });
    document.body.append(t);
    t.select();
    const ok = document.execCommand('copy');
    t.remove();
    if (!ok) throw new Error('copie impossible');
  }
}

// Formes d'onde deja calculees : generation + version (originale ou amelioree) -> crêtes.
const waves = new Map();
async function drawBars(box, id, enhanced) {
  const key = `${id}:${enhanced}`;
  let peaks = waves.get(key);
  if (!peaks) {
    try { peaks = await invoke('waveform', { kind: 'generation', id }); waves.set(key, peaks); } catch { peaks = []; }
  }
  box.replaceChildren(...peaks.map((p) => { const bar = el('i'); bar.style.height = `${4 + Math.round(p * 0.26)}px`; return bar; }));
}

async function loadGenerations() {
  const st = $('status');
  const list = await invoke('generations');
  $('gen-count').textContent = list.length ? `${list.length} audio${list.length > 1 ? 's' : ''}` : '';
  $('gen-list').replaceChildren(...list.map((g) => {
    const copy = el('button', { type: 'button', className: 'ghost icon', title: 'Copier le texte' });
    copy.setAttribute('aria-label', 'Copier le texte de cette génération');
    copy.append(icon(...ICON_COPY));
    copy.onclick = async () => {
      try {
        await copyText(g.text);
        copy.classList.add('done');
        copy.replaceChildren(icon(...ICON_CHECK));
        say(st, 'Texte copié.');
        setTimeout(() => { copy.classList.remove('done'); copy.replaceChildren(icon(...ICON_COPY)); }, 1500);
      } catch (e) { fail(st, e); }
    };
    const folder = el('button', { type: 'button', className: 'ghost', textContent: 'Dossier' });
    folder.setAttribute('aria-label', 'Montrer le fichier dans l\'Explorateur');
    folder.onclick = () => invoke('reveal', { id: g.id }).catch((e) => fail(st, e));
    // Original | Amélioré : l'amélioration est un égaliseur de voix + une normalisation du volume.
    const version = (label, on, title) => {
      const b = el('button', { type: 'button', textContent: label, title });
      b.setAttribute('aria-pressed', g.enhanced === on);
      b.onclick = async () => {
        if (g.enhanced === on) return;
        b.disabled = true;
        try {
          // La version jouée change : on coupe la lecture de cette génération avant de la remplacer.
          if (playingKey === `generation:${g.id}`) $('player').pause();
          await invoke('set_enhanced', { id: g.id, on });
          say(st, on ? 'Amélioration activée : voix égalisée et volume normalisé.' : 'Audio d\'origine rétabli.');
          await loadGenerations();
        } catch (e) { b.disabled = false; fail(st, e); }
      };
      return b;
    };
    const seg = el('div', { className: 'seg' }, version('Original', false, 'Audio tel que généré'), version('Amélioré', true, 'Égaliseur de voix et volume normalisé'));
    seg.setAttribute('role', 'group');
    seg.setAttribute('aria-label', 'Version de l\'audio');
    const del = el('button', { type: 'button', className: 'ghost danger icon end', title: 'Supprimer cette génération' });
    del.append(icon(...ICON_TRASH));
    del.setAttribute('aria-label', 'Supprimer cette génération');
    del.onclick = async () => {
      if (!confirm('Supprimer cette génération et son fichier audio ? Cette action est définitive.')) return;
      try { await invoke('delete_generation', { id: g.id }); await loadGenerations(); } catch (e) { fail(st, e); }
    };
    const dl = (format) => {
      const b = el('button', { type: 'button', className: 'ghost', textContent: format.toUpperCase() });
      b.setAttribute('aria-label', `Télécharger en ${format.toUpperCase()}${g.enhanced ? ' (version améliorée)' : ''}`);
      b.onclick = async () => {
        b.disabled = true;
        say(st, `Choisissez le dossier et le nom du ${format.toUpperCase()}…`);
        try {
          const path = await invoke('download', { id: g.id, format });
          say(st, path ? `Enregistré : ${path}` : 'Téléchargement annulé.');
        } catch (e) { fail(st, e); }
        b.disabled = false;
      };
      return b;
    };
    const bars = el('div', { className: 'bars' });
    bars.dataset.key = `generation:${g.id}`;
    bars.setAttribute('aria-hidden', 'true');
    drawBars(bars, g.id, g.enhanced);
    return el('li', { className: 'gen' },
      el('div', { className: 'gen-top' }, el('p', { className: 'gen-text', textContent: g.text, title: g.text }), copy),
      el('div', { className: 'audio' },
        el('div', { className: 'player' }, playButton('generation', g.id, st), bars, el('span', { className: 'dur', textContent: fmt(g.duration_sec) })),
        el('div', { className: 'meta' },
          el('span', { className: 'voice', textContent: `Voix : ${g.profile ?? 'supprimée'}` }),
          g.created_at.slice(0, 16)),
        el('div', { className: 'actions' }, seg, folder, dl('wav'), dl('mp3'), del)));
  }));
  if (!list.length) $('gen-list').append(el('li', { className: 'empty', textContent: 'Les audios générés apparaîtront ici, y compris ceux demandés par Claude Code.' }));
  markPlaying();
}

// ---------------------------------------------------------------- parametres : carte graphique
const gpuLabel = (d) => `${d.name} · ${d.description}${d.gpu && d.total_mb ? ` · ${(d.total_mb / 1024).toFixed(0)} Go` : ''}`;
const TRI = { auto: 'Automatique (recommandé)', on: 'Activée', off: 'Désactivée' };
for (const id of ['gpu-fa', 'gpu-clamp']) $(id).append(...Object.entries(TRI).map(([value, textContent]) => el('option', { value, textContent })));

async function loadGpu() {
  const st = $('gpu-status');
  const g = await invoke('gpu');
  const byName = (n) => g.devices.find((d) => d.name === n);
  const best = byName(g.best);
  const choice = g.choice !== 'auto' && byName(g.choice) ? g.choice : 'auto';
  const target = choice === 'auto' ? best : byName(choice);

  $('gpu-summary').textContent = best
    ? `Meilleur choix détecté : ${gpuLabel(best)}. ${g.active ? `Calcul en cours sur ${g.active}.` : 'Le modèle est chargé à la prochaine génération.'}`
    : 'Aucun périphérique de calcul détecté.';
  $('gpu-device').replaceChildren(
    el('option', { value: 'auto', textContent: `Automatique : ${best ? best.name : 'processeur'} (recommandé)` }),
    ...g.devices.map((d) => el('option', { value: d.name, textContent: gpuLabel(d) + (d.recommended ? ' · recommandé' : '') })));
  $('gpu-device').value = choice;
  $('gpu-fa').value = g.fa;
  $('gpu-clamp').value = g.clamp;

  // Un choix manuel different du meilleur : on le dit et on propose de revenir au choix automatique.
  const hint = $('gpu-best-note');
  hint.hidden = !(choice !== 'auto' && best && target && target.name !== best.name);
  hint.textContent = hint.hidden ? '' : `Le meilleur choix détecté est ${best.name} (${best.description}). Choisissez « Automatique » pour l'utiliser.`;
  $('gpu-nvidia').hidden = !g.nvidia_without_cuda;
  $('gpu-note').textContent = g.note ? `Repli automatique : ${g.note}.` : '';
  $('gpu-advanced').open ||= g.fa !== 'auto' || g.clamp !== 'auto';
  say(st, '');
}
async function saveGpu() {
  const st = $('gpu-status');
  try {
    await invoke('set_gpu', { device: $('gpu-device').value, fa: $('gpu-fa').value, clamp: $('gpu-clamp').value });
    await loadGpu();
    say(st, 'Enregistré. Le modèle sera rechargé avec ce réglage à la prochaine génération.');
  } catch (e) { fail(st, e); await loadGpu().catch(() => {}); }
}
for (const id of ['gpu-device', 'gpu-fa', 'gpu-clamp']) $(id).onchange = saveGpu;

// ---------------------------------------------------------------- parametres : modeles
const progress = {}; // fichier -> { done, total } pendant un telechargement
const size = (mb) => (mb >= 1000 ? `${(mb / 1000).toFixed(1).replace('.', ',')} Go` : `${mb} Mo`);
const percent = (p) => (p.total ? Math.floor((p.done / p.total) * 100) : 0);

async function loadModels() {
  const st = $('models-status');
  const m = await invoke('models');
  $('models-source').textContent = m.source.replace('https://', '').replace('/resolve/main/', '');
  $('models-dir').textContent = `Dossier : ${m.dir}`;
  $('memory-state').textContent = m.loaded
    ? 'Un modèle est chargé en mémoire (environ 2,5 Go de RAM, plus la mémoire de la carte graphique). Le décharger libère cette mémoire ; il sera rechargé à la prochaine génération.'
    : 'Aucun modèle en mémoire. Il sera chargé à la prochaine génération.';
  $('unload').disabled = !m.loaded;
  $('unload').onclick = () => invoke('unload_model').then(() => Promise.all([loadModels(), loadGpu()])).then(() => say($('models-status'), 'Modèle déchargé.'), (e) => fail($('models-status'), e));
  $('model-list').replaceChildren(...m.models.map((x) => {
    const state = [x.downloaded ? (x.active ? 'utilisé pour les générations' : 'installé') : 'non installé', x.loaded ? 'en mémoire' : ''].filter(Boolean).join(' · ');
    const row = el('li', { className: `model${x.active || (x.codec && x.downloaded) ? ' active' : ''}` },
      el('div', { className: 'grow' }, el('strong', { textContent: x.label }), el('span', { className: 'meta', textContent: `${size(x.size_mb)} · ${state}` })));
    const act = (label, fn, cls = '') => {
      const b = el('button', { type: 'button', className: cls, textContent: label });
      b.setAttribute('aria-label', `${label} : ${x.label}`);
      b.onclick = () => fn().catch((e) => fail(st, e));
      row.append(b);
    };
    if (x.downloading || progress[x.file]) {
      const p = progress[x.file] ?? { done: 0, total: 0 };
      const bar = el('progress', { max: 100, value: percent(p) });
      bar.dataset.file = x.file;
      bar.setAttribute('aria-label', `Téléchargement de ${x.label}`);
      const pct = el('span', { className: 'meta', textContent: `${percent(p)} %` });
      pct.dataset.pct = x.file;
      row.append(bar, pct);
      act('Annuler', () => invoke('cancel_download', { file: x.file }));
    } else if (!x.downloaded) {
      act(`Télécharger (${size(x.size_mb)})`, async () => {
        progress[x.file] = { done: 0, total: 0 };
        say(st, `Téléchargement de ${x.label}…`);
        invoke('download_model', { file: x.file })
          .then(() => say(st, `${x.label} est installé.`), (e) => fail(st, e))
          .finally(() => { delete progress[x.file]; loadModels().catch((e) => fail(st, e)); });
        await loadModels();
      });
    } else {
      if (!x.codec && !x.active) act('Utiliser', async () => { await invoke('set_model', { file: x.file }); await loadModels(); say(st, `${x.label} sera utilisé à la prochaine génération.`); });
      act('Supprimer', async () => {
        if (!confirm(`Supprimer ${x.label} (${size(x.size_mb)}) du disque ? Il faudra le retélécharger pour s'en resservir.`)) return;
        await invoke('delete_model', { file: x.file });
        await loadModels();
        say(st, `${x.label} est supprimé.`);
      }, 'ghost danger');
    }
    return row;
  }));
}

$('open-models').onclick = () => invoke('open_models_dir').catch((e) => fail($('models-status'), e));

// ---------------------------------------------------------------- parametres : apparence
const THEMES = [
  { id: 'nocturne', name: 'Nocturne', desc: 'Sombre, accent ambre', colors: ['#0f1217', '#1d2330', '#ffb03b'] },
  { id: 'atelier', name: 'Atelier', desc: 'Clair, papier crème, titres en serif', colors: ['#f6f1e7', '#1d1b17', '#c2410c'] },
  { id: 'console', name: 'Console', desc: 'Graphite et turquoise, style studio audio', colors: ['#0b0d0e', '#182022', '#2dd4bf'] },
  { id: 'conversation', name: 'Conversation', desc: 'Clair et arrondi, générations en bulles de discussion', colors: ['#f1f2f7', '#ffffff', '#4338ca'] }
];
function applyTheme(id) {
  if (id === 'nocturne') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = id;
  try { localStorage.setItem('voiceclone-theme', id); } catch { /* choix valable pour cette session seulement */ }
}
{
  const saved = document.documentElement.dataset.theme;
  const now = THEMES.some((t) => t.id === saved) ? saved : 'nocturne';
  if (saved && now === 'nocturne') delete document.documentElement.dataset.theme; // valeur inconnue : on l'ecarte
  $('themes').append(...THEMES.map((t) => {
    const input = el('input', { type: 'radio', name: 'theme', value: t.id, checked: t.id === now });
    input.onchange = () => applyTheme(t.id);
    const swatch = el('span', { className: 'swatch' }, ...t.colors.map((c) => { const i = el('i'); i.style.background = c; return i; }));
    return el('label', { className: 'theme' }, input, swatch, el('strong', { textContent: t.name }), el('span', { className: 'desc', textContent: t.desc }));
  }));
}

function show(page) {
  const settings = page === 'settings';
  $('settings').hidden = !settings;
  $('studio').hidden = settings;
  $('nav-settings').setAttribute('aria-pressed', settings);
  $('nav-studio').setAttribute('aria-pressed', !settings);
  if (settings) {
    loadModels().catch((e) => fail($('models-status'), e));
    loadGpu().catch((e) => fail($('gpu-status'), e));
  }
}
$('nav-studio').onclick = () => show('studio');
$('nav-settings').onclick = () => show('settings');

// ---------------------------------------------------------------- demarrage
(async () => {
  try {
    await listen('model-progress', ({ payload: p }) => {
      if (!progress[p.file]) return; // telechargement deja termine ou annule
      progress[p.file] = p;
      const bar = document.querySelector(`progress[data-file="${p.file}"]`), pct = document.querySelector(`[data-pct="${p.file}"]`);
      if (bar) bar.value = percent(p);
      if (pct) pct.textContent = `${percent(p)} % · ${size(Math.round(p.done / 1048576))} sur ${size(Math.round(p.total / 1048576))}`;
    });
    const mcp = await invoke('mcp_info');
    $('mcp-text').textContent = mcp.error ? `Connecteur MCP arrêté : ${mcp.error}` : `Connecteur MCP actif · ${mcp.url}`;
    $('mcp').classList.toggle('error', !!mcp.error);
    await listen('changed', () => loadGenerations().catch((e) => fail($('status'), e)));
    await loadProfiles();
    await loadGenerations();
  } catch (e) { fail($('status'), e); }
})();
