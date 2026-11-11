// Interface VoiceClone : tout le texte passe par textContent (jamais innerHTML) et par t() (voir i18n.js).
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);

const LANG_CODES = ['french', 'english', 'german', 'spanish', 'italian', 'portuguese', 'japanese', 'korean', 'chinese', 'russian'];
const langName = (code) => (LANG_CODES.includes(code) ? t(`lang.${code}`) : code);
const WARN_AFTER_SEC = 90; // mesure : a 3 min de reference la generation deraille, a 1 min 30 elle tient

let profiles = [], current = null, samples = [], busy = false, recorder = null, playingKey = null, page = 'studio';
let appVersion = '', updateInfo = null;

const lsGet = (k) => { try { return localStorage.getItem(k); } catch { return null; } };
const lsSet = (k, v) => { try { localStorage.setItem(k, v); } catch { /* preference valable pour cette session seulement */ } };

function el(tag, props = {}, ...kids) {
  const n = Object.assign(document.createElement(tag), props);
  n.append(...kids);
  return n;
}
const fmt = (s) => { s = Math.round(s); return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`; };
const say = (where, msg) => { where.textContent = msg; where.classList.remove('error'); };
const fail = (where, err) => { where.textContent = t('common.error', { msg: err?.message ?? err }); where.classList.add('error'); };
const profile = () => profiles.find((p) => p.id === current);
// Taille lisible : « 2,0 Go » en francais, « 2.0 GB » en anglais.
const size = (mb) => (mb >= 1000 ? `${(mb / 1000).toFixed(1).replace('.', LANG === 'fr' ? ',' : '.')} ${t('unit.gb')}` : `${Math.round(mb)} ${t('unit.mb')}`);
const percent = (p) => (p.total ? Math.floor((p.done / p.total) * 100) : 0);

// ---------------------------------------------------------------- lecture audio
const PLAY = 'M7 4.5v15l13-7.5z', PAUSE = 'M7 4h4v16H7zM13 4h4v16h-4z';
const initials = (name) => name.trim().split(/\s+/).map((w) => w[0]).join('').slice(0, 2).toUpperCase();

function markPlaying() {
  for (const b of document.querySelectorAll('button[data-key]')) {
    const on = b.dataset.key === playingKey;
    if (b.classList.contains('play-circle')) {
      b.replaceChildren(icon(on ? PAUSE : PLAY));
      b.setAttribute('aria-label', on ? t('common.stop_playback') : t('common.play'));
    } else {
      b.textContent = on ? t('common.stop') : t('common.play');
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
        if (kind === 'generation') {
          b.closest('.gen')?.classList.remove('unheard');
          invoke('mark_heard', { id }).catch(() => {});
        }
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
      voiceAvatar(p),
      el('span', { className: 'who' },
        el('strong', { textContent: p.name }),
        el('span', { textContent: tp('voice.meta', { lang: langName(p.language), n: p.samples }) })));
    pick.setAttribute('aria-pressed', p.id === current);
    const del = el('button', { type: 'button', className: 'ghost danger icon', title: t('voice.delete') });
    del.append(icon(...ICON_TRASH));
    del.setAttribute('aria-label', t('voice.delete_aria', { name: p.name }));
    del.onclick = async () => {
      if (!(await ask(t('voice.delete_confirm', { name: p.name })))) return;
      try { await invoke('delete_profile', { id: p.id }); await loadProfiles(); await loadGenerations(); } catch (e) { fail($('status'), e); }
    };
    return el('li', {}, pick, del);
  }));
  renderTags();
  if (!$('settings').hidden) renderVoicesTab();
  if (!profiles.length) $('new-profile-box').open = true;
  if (!profiles.length) $('profile-list').append(el('li', { className: 'empty', textContent: t('voices.empty') }));
  await loadSamples();
  updateGo();
}

function fillLanguageOptions() {
  const keep = $('np-lang').value;
  $('np-lang').replaceChildren(...LANG_CODES.map((value) => el('option', { value, textContent: t(`lang.${value}`) })));
  if (keep) $('np-lang').value = keep;
}
$('new-profile').onsubmit = async (ev) => {
  ev.preventDefault();
  try {
    current = await invoke('create_profile', { name: $('np-name').value, language: $('np-lang').value });
    $('np-name').value = '';
    $('new-profile-box').open = false;
    await loadProfiles();
    say($('sample-status'), t('voices.created'));
  } catch (e) { fail($('status'), e); }
};

// ---------------------------------------------------------------- echantillons
function sampleNotes() {
  const total = samples.reduce((s, x) => s + x.duration_sec, 0);
  const notes = [];
  if (!samples.length) notes.push(t('notes.none'));
  for (const code of quality.voice_issues) notes.push(QMSG[code]());
  if (samples.length && !quality.voice_issues.length && samples.every((s) => !(quality.samples[s.id]?.issues.length))) notes.push(t('q.good_voice'));
  if (total > WARN_AFTER_SEC) notes.push(t('notes.long', { total: fmt(total), limit: fmt(WARN_AFTER_SEC) }));
  $('sample-notes').textContent = notes.join(' ');
}

// ---------------------------------------------------------------- assistant de qualite des voix
const QMSG = {
  short: () => t('q.short'), clipping: () => t('q.clipping'), quiet: () => t('q.quiet'), noisy: () => t('q.noisy'),
  no_text: () => t('q.no_text'), mismatch: () => t('q.mismatch'), pitch_mix: () => t('q.pitch_mix')
};
let quality = { samples: {}, voice_issues: [] };
async function loadQuality(p) {
  try {
    const q = await invoke('voice_quality', { profileId: p.id });
    quality = { samples: Object.fromEntries(q.samples.map((x) => [x.id, x])), voice_issues: q.voice_issues };
  } catch { quality = { samples: {}, voice_issues: [] }; }
}
function qualityBlock(s) {
  const q = quality.samples[s.id];
  if (!q) return '';
  return el('div', { className: 'q' },
    el('p', { className: 'q-metrics', textContent: t('q.metrics', { f0: q.f0 ?? '?', snr: q.snr_db, peak: q.peak }) }),
    ...q.issues.map((code) => el('p', { className: 'q-issue', textContent: QMSG[code]() })),
    ...(q.issues.length ? [] : [el('p', { className: 'q-ok', textContent: t('q.ok') })]));
}

async function loadSamples() {
  const p = profile();
  $('samples-panel').hidden = !p;
  if (!p) return;
  $('samples-title').textContent = t('samples.title', { name: p.name });
  samples = await invoke('samples', { profileId: p.id });
  await loadQuality(p);
  const st = $('sample-status');
  $('sample-list').replaceChildren(...samples.map((s, i) => {
    const n = i + 1;
    const text = el('textarea', { value: s.ref_text, rows: 3, placeholder: t('sample.placeholder') });
    text.setAttribute('aria-label', t('sample.transcript_aria', { n }));
    text.onchange = async () => {
      try { await invoke('set_sample_text', { id: s.id, text: text.value }); await loadSamples(); say(st, t('sample.saved')); }
      catch (e) { fail(st, e); }
    };
    const del = el('button', { type: 'button', className: 'ghost danger', textContent: t('common.delete') });
    del.setAttribute('aria-label', t('sample.delete_aria', { n }));
    del.onclick = async () => {
      if (!(await ask(t('sample.delete_confirm', { n })))) return;
      try { await invoke('delete_sample', { id: s.id }); await loadProfiles(); } catch (e) { fail(st, e); }
    };
    return el('li', { className: 'sample' },
      el('div', { className: 'row' }, el('span', { className: 'grow', textContent: t('sample.label', { n, dur: fmt(s.duration_sec) }) }), playButton('sample', s.id, st), del),
      text, qualityBlock(s));
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
  say(st, t('sample.converting'));
  const pcm = await to24k(arrayBuffer);
  const id = await invoke('add_sample', new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength), { headers: { 'x-profile': String(profileId) } });
  if (transcript) await invoke('set_sample_text', { id, text: transcript });
  await loadProfiles();
  say(st, transcript ? t('sample.added_guided') : t('sample.added'));
}

// Texte propose a la lecture au micro : celui de la langue de la voix (il sert ensuite de transcription).
const readingText = (lang) => (lang === 'french' ? t('readme.french') : lang === 'english' ? t('readme.english') : '');

$('rec').onclick = async () => {
  const st = $('sample-status'), btn = $('rec');
  if (recorder) { recorder.stop(); return; }
  const p = profile();
  let stream;
  try { stream = await navigator.mediaDevices.getUserMedia({ audio: true }); }
  catch (e) { fail(st, t('sample.mic_error', { msg: e.message })); return; }
  const chunks = [], t0 = Date.now();
  const script = readingText(p.language);
  const tick = () => { $('rec-label').textContent = t('samples.stop_rec', { time: fmt((Date.now() - t0) / 1000) }); };
  const timer = setInterval(tick, 500);
  recorder = new MediaRecorder(stream);
  recorder.ondataavailable = (e) => chunks.push(e.data);
  recorder.onstop = async () => {
    stream.getTracks().forEach((tr) => tr.stop());
    clearInterval(timer);
    recorder = null;
    $('rec-label').textContent = t('samples.rec');
    btn.classList.remove('recording');
    $('read-me').hidden = true;
    try { await addSample(await new Blob(chunks).arrayBuffer(), p.id, script); } catch (e) { fail(st, e); }
  };
  recorder.start();
  btn.classList.add('recording');
  tick();
  $('read-me').replaceChildren(el('strong', { textContent: t('readme.prefix') }), script);
  $('read-me').hidden = !script;
  say(st, script ? t('sample.rec_guided') : t('sample.rec_plain'));
};

$('import').onclick = () => $('file').click();
$('file').onchange = async (ev) => {
  const file = ev.target.files[0];
  ev.target.value = '';
  if (!file) return;
  try { await addSample(await file.arrayBuffer(), current, ''); }
  catch (e) { fail($('sample-status'), t('sample.file_error', { msg: e?.message ?? e })); }
};

// ---------------------------------------------------------------- balises : insertion au curseur
// Chaque voix a sa couleur (teinte fixe selon son numero ; clarte et saturation viennent du theme : --vs, --vl).
const VOICE_HUES = [205, 335, 140, 275, 28, 172, 52, 95];
// Teinte posee par l'API CSSOM : la CSP de l'application interdit l'attribut HTML style (donc pas de setAttribute).
const paint = (node, p) => { if (p) node.style.setProperty('--h', VOICE_HUES[(p.id - 1) % VOICE_HUES.length]); return node; };
function voiceAvatar(p) {
  const a = el('span', { className: 'avatar vc-bg', textContent: initials(p.name) });
  return paint(a, p);
}
// Decoupe le texte comme le programme : [pause], [NomDeVoix] (change de voix), reste = texte dit par la voix courante.
function voiceSpans(text) {
  const out = [];
  let cur = profile(), last = 0;
  const say = (s) => s && out.push({ s, cls: 'vc', p: cur });
  for (const m of text.matchAll(/\[[^\]\n[]{1,60}\]/g)) {
    say(text.slice(last, m.index));
    const name = m[0].slice(1, -1).trim();
    if (/^pause(\s|$)/i.test(name)) out.push({ s: m[0], cls: 'hl-pause' });
    else {
      const v = profiles.find((p) => p.name.toLowerCase() === name.toLowerCase());
      if (v) cur = v;
      out.push(v ? { s: m[0], cls: 'vc hl-tag', p: v } : { s: m[0], cls: 'hl-bad' });
    }
    last = m.index + m[0].length;
  }
  say(text.slice(last));
  return out;
}
// Le texte est repeint par-dessus la zone de saisie (transparente) avec la couleur de chaque voix.
function renderHighlight() {
  const tx = $('text'), hl = $('hl'), cs = getComputedStyle(tx);
  for (const k of ['font', 'lineHeight', 'letterSpacing', 'textAlign', 'padding', 'borderTopWidth', 'borderTopLeftRadius']) hl.style[k] = cs[k];
  hl.style.borderStyle = 'solid';
  hl.style.borderColor = 'transparent';
  hl.style.width = `${tx.clientWidth + parseFloat(cs.borderLeftWidth) * 2}px`;
  const spans = voiceSpans(tx.value).map((x) => {
    const e = el('span', { className: x.cls, textContent: x.s });
    return paint(e, x.p);
  });
  $('hl-in').replaceChildren(...spans, document.createTextNode(tx.value.endsWith('\n') ? ' ' : ''));
  $('hl-in').style.transform = `translateY(${-tx.scrollTop}px)`;
}
new ResizeObserver(() => renderHighlight()).observe($('text'));
$('text').addEventListener('scroll', () => { $('hl-in').style.transform = `translateY(${-$('text').scrollTop}px)`; });
$('text').addEventListener('input', renderHighlight);

function insertTag(tag) {
  const tx = $('text'), a = tx.selectionStart ?? tx.value.length, b = tx.selectionEnd ?? a;
  const before = tx.value.slice(0, a), pad = before && !/\s$/.test(before) ? ' ' : '';
  tx.setRangeText(`${pad}${tag} `, a, b, 'end');
  tx.focus();
  tx.dispatchEvent(new Event('input'));
}
function renderTags() {
  $('tags').replaceChildren(
    el('button', { type: 'button', className: 'tag pause', textContent: t('tags.pause'), title: t('tags.pause_tip'), onclick: () => insertTag('[pause]') }),
    ...profiles.map((p) => paint(el('button', { type: 'button', className: 'tag voice vc', textContent: p.name, title: t('tags.voice_tip', { name: p.name }), onclick: () => insertTag(`[${p.name}]`) }), p)));
  renderHighlight();
}

// ---------------------------------------------------------------- generation
function updateGo() {
  const p = profile(), n = $('text').value.trim().length;
  $('go').disabled = busy || !p || !p.samples || !n;
  $('clear').disabled = !$('text').value;
  $('count').replaceChildren(...(p
    ? [voiceAvatar(p), tp('compose.line', { name: p.name, n })]
    : [t('compose.no_voice')]));
}

async function generate() {
  const p = profile(), text = $('text').value.trim(), st = $('status');
  if (busy || !p || !text) return;
  busy = true;
  updateGo();
  say(st, t('compose.generating'));
  try {
    const r = await invoke('speak', { text, profile: p.name });
    say(st, [t('compose.done', { dur: fmt(r.duration_sec), s: r.elapsed_sec }), ...(r.warnings ?? [])].join(' '));
    if (r.warnings?.length) st.classList.add('error');
  } catch (e) { fail(st, e); }
  busy = false;
  updateGo();
}
$('go').onclick = generate;
$('text').oninput = updateGo;
$('text').onkeydown = (ev) => { if (ev.ctrlKey && ev.key === 'Enter') generate(); };
// Efface par une commande d'edition : Ctrl+Z ramene le texte (une affectation de .value l'effacerait pour de bon).
$('clear').onclick = () => {
  const tx = $('text');
  tx.focus();
  tx.select();
  if (!document.execCommand('delete')) tx.value = '';
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
    const tx = el('textarea', { value: text });
    document.body.append(tx);
    tx.select();
    const ok = document.execCommand('copy');
    tx.remove();
    if (!ok) throw new Error(t('gen.copy_failed'));
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

let genTotal = 0;
$('gen-clear').append(icon(...ICON_TRASH));
$('gen-clear').onclick = async () => {
  if (!genTotal || !(await ask(t('gen.clear_confirm', { n: genTotal })))) return;
  if (playingKey?.startsWith('generation:')) $('player').pause();
  try { await invoke('delete_all_generations'); await loadGenerations(); say($('status'), t('gen.cleared')); } catch (e) { fail($('status'), e); }
};

async function loadGenerations() {
  const st = $('status');
  const list = await invoke('generations');
  $('gen-count').textContent = list.length ? tp('gen.count', { n: list.length }) : '';
  $('gen-clear').disabled = !list.length;
  genTotal = list.length;
  $('gen-list').replaceChildren(...list.map((g) => {
    const copy = el('button', { type: 'button', className: 'ghost icon', title: t('gen.copy') });
    copy.setAttribute('aria-label', t('gen.copy_aria'));
    copy.append(icon(...ICON_COPY));
    copy.onclick = async () => {
      try {
        await copyText(g.text);
        copy.classList.add('done');
        copy.replaceChildren(icon(...ICON_CHECK));
        say(st, t('gen.copied'));
        setTimeout(() => { copy.classList.remove('done'); copy.replaceChildren(icon(...ICON_COPY)); }, 1500);
      } catch (e) { fail(st, e); }
    };
    const folder = el('button', { type: 'button', className: 'ghost', textContent: t('gen.folder') });
    folder.setAttribute('aria-label', t('gen.folder_aria'));
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
          say(st, on ? t('gen.enh_on') : t('gen.enh_off'));
          await loadGenerations();
        } catch (e) { b.disabled = false; fail(st, e); }
      };
      return b;
    };
    const seg = el('div', { className: 'seg' }, version(t('gen.original'), false, t('gen.original_title')), version(t('gen.enhanced'), true, t('gen.enhanced_title')));
    seg.setAttribute('role', 'group');
    seg.setAttribute('aria-label', t('gen.version_aria'));
    const del = el('button', { type: 'button', className: 'ghost danger icon end', title: t('gen.delete') });
    del.append(icon(...ICON_TRASH));
    del.setAttribute('aria-label', t('gen.delete'));
    del.onclick = async () => {
      if (!(await ask(t('gen.delete_confirm')))) return;
      try { await invoke('delete_generation', { id: g.id }); await loadGenerations(); } catch (e) { fail(st, e); }
    };
    const dl = (format) => {
      const b = el('button', { type: 'button', className: 'ghost', textContent: format.toUpperCase() });
      b.setAttribute('aria-label', t(g.enhanced ? 'gen.dl_aria_enh' : 'gen.dl_aria', { fmt: format.toUpperCase() }));
      b.onclick = async () => {
        b.disabled = true;
        say(st, t('gen.dl_choose', { fmt: format.toUpperCase() }));
        try {
          const path = await invoke('download', { id: g.id, format });
          say(st, path ? t('gen.saved', { path }) : t('gen.cancelled'));
        } catch (e) { fail(st, e); }
        b.disabled = false;
      };
      return b;
    };
    const bars = el('div', { className: 'bars' });
    bars.dataset.key = `generation:${g.id}`;
    bars.setAttribute('aria-hidden', 'true');
    drawBars(bars, g.id, g.enhanced);
    return el('li', { className: g.heard ? 'gen' : 'gen unheard' },
      el('div', { className: 'gen-top' }, el('p', { className: 'gen-text', textContent: g.text, title: g.text }), copy),
      el('div', { className: 'audio' },
        el('div', { className: 'player' }, playButton('generation', g.id, st), bars, el('span', { className: 'dur', textContent: fmt(g.duration_sec) })),
        el('div', { className: 'meta' },
          el('span', { className: 'new-badge', textContent: t('gen.new') }),
          g.multi
            ? el('span', { className: 'voice multi', textContent: t('gen.voice', { name: t('gen.multiple') }) })
            : paint(el('span', { className: 'voice vc', textContent: t('gen.voice', { name: g.profile ?? t('gen.voice_deleted') }) }), profiles.find((p) => p.name === g.profile)),
          g.created_at.slice(0, 16)),
        el('div', { className: 'actions' }, seg, folder, dl('wav'), dl('mp3'), del)));
  }));
  if (!list.length) $('gen-list').append(el('li', { className: 'empty', textContent: t('gen.empty') }));
  markPlaying();
}

// ---------------------------------------------------------------- confirmation (a la place de confirm() : son titre serait l'adresse interne)
function ask(msg) {
  return new Promise((resolve) => {
    const d = $('ask');
    $('ask-msg').textContent = msg;
    d.returnValue = 'cancel';
    d.onclose = () => resolve(d.returnValue === 'ok');
    d.showModal();
    $('ask-ok').focus();
  });
}

// ---------------------------------------------------------------- parametres : onglets
const TAB_LABELS = { interface: () => t('tab.interface'), voices: () => t('tab.voices'), connect: () => t('tab.connect'), audios: () => t('tab.audios'), models: () => t('tab.models'), gpu: () => t('tab.gpu'), updates: () => t('tab.updates') };
const SETTINGS_TABS = Object.keys(TAB_LABELS);
let settingsTab = 'interface';
try { const saved = localStorage.getItem('vc-settings-tab'); if (SETTINGS_TABS.includes(saved)) settingsTab = saved; } catch { /* stockage indisponible */ }
function selectTab(id, focus = false) {
  settingsTab = id;
  try { localStorage.setItem('vc-settings-tab', id); } catch { /* idem */ }
  renderTabs();
  if (focus) $(`tab-btn-${id}`).focus();
}
function renderTabs() {
  $('settings-tabs').replaceChildren(...SETTINGS_TABS.map((id, i) => {
    const b = el('button', { type: 'button', id: `tab-btn-${id}`, className: 'tab', textContent: TAB_LABELS[id]() });
    b.setAttribute('role', 'tab');
    b.setAttribute('aria-selected', id === settingsTab);
    b.setAttribute('aria-controls', `tab-${id}`);
    b.tabIndex = id === settingsTab ? 0 : -1;
    b.onclick = () => selectTab(id);
    b.onkeydown = (ev) => {
      const n = SETTINGS_TABS.length;
      const next = { ArrowRight: (i + 1) % n, ArrowDown: (i + 1) % n, ArrowLeft: (i + n - 1) % n, ArrowUp: (i + n - 1) % n, Home: 0, End: n - 1 }[ev.key];
      if (next !== undefined) { ev.preventDefault(); selectTab(SETTINGS_TABS[next], true); }
    };
    return b;
  }));
  for (const id of SETTINGS_TABS) $(`tab-${id}`).hidden = id !== settingsTab;
}
renderTabs();

// ---------------------------------------------------------------- parametres : langue et apparence
const THEMES = [
  { id: 'nocturne', colors: ['#0f1217', '#1d2330', '#ffb03b'] },
  { id: 'atelier', colors: ['#f6f1e7', '#1d1b17', '#c2410c'] },
  { id: 'console', colors: ['#0b0d0e', '#182022', '#2dd4bf'] },
  { id: 'conversation', colors: ['#f1f2f7', '#ffffff', '#4338ca'] }
];
function applyTheme(id) {
  requestAnimationFrame(() => renderHighlight());
  if (id === 'nocturne') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = id;
  try { localStorage.setItem('voiceclone-theme', id); } catch { /* choix valable pour cette session seulement */ }
}
function renderThemeChoices() {
  const saved = document.documentElement.dataset.theme;
  const now = THEMES.some((x) => x.id === saved) ? saved : 'nocturne';
  if (saved && now === 'nocturne') delete document.documentElement.dataset.theme; // valeur inconnue : on l'ecarte
  $('themes').replaceChildren(...THEMES.map((th) => {
    const input = el('input', { type: 'radio', name: 'theme', value: th.id, checked: th.id === now });
    input.onchange = () => applyTheme(th.id);
    const swatch = el('span', { className: 'swatch' }, ...th.colors.map((c) => { const i = el('i'); i.style.background = c; return i; }));
    return el('label', { className: 'theme' }, input, swatch, el('strong', { textContent: t(`theme.${th.id}`) }), el('span', { className: 'desc', textContent: t(`theme.${th.id}_desc`) }));
  }));
}
function renderLanguageChoices() {
  $('languages').replaceChildren(...['fr', 'en'].map((code) => {
    const input = el('input', { type: 'radio', name: 'ui-language', value: code, checked: code === LANG });
    input.onchange = () => changeLanguage(code);
    return el('label', { className: 'theme lang' }, input, el('strong', { textContent: t(`lang.ui.${code}`) }));
  }));
}

// Changer de langue : textes statiques, puis tout ce que main.js a construit, puis les messages venant de Rust.
async function changeLanguage(code) {
  setLanguage(code);
  try { await invoke('set_lang', { lang: code }); } catch (e) { fail($('models-status'), e); }
  await renderAll();
}

// ---------------------------------------------------------------- parametres : connecter une IA ou un script
// Les chemins viennent du programme en cours (connection_info) : ils suivent l'endroit ou VoiceClone est installe.
let connInfo = null;

function codeBlock(title, text, note) {
  const copy = el('button', { type: 'button', className: 'ghost', textContent: t('conn.copy') });
  copy.setAttribute('aria-label', t('conn.copy_aria', { title }));
  copy.onclick = async () => {
    try {
      await copyText(text);
      copy.textContent = t('conn.copied');
      say($('conn-status'), t('conn.copied_status'));
      setTimeout(() => { copy.textContent = t('conn.copy'); }, 1500);
    } catch (e) { fail($('conn-status'), e); }
  };
  return el('div', { className: 'codeblock' },
    el('div', { className: 'row head' }, el('strong', { textContent: title }), copy),
    el('pre', { textContent: text }),
    note ? el('p', { className: 'hint', textContent: note }) : '');
}

function renderConnection() {
  const c = connInfo;
  if (!c) return;
  const hasServer = !!c.server;
  $('conn-warn').hidden = hasServer;
  const server = c.server ?? 'voiceclone-server.exe';
  const v = { url: c.url, server, voice: profiles[0]?.name ?? 'Zen', lang: t('conn.sample_lang'), sample: t('conn.sample'), downloads: c.downloads ?? '%USERPROFILE%\\Downloads' };
  const cli = (prefix, cmt) => [
    `${cmt} ${t('conn.cmt_std')}`, `${prefix}"${server}" speak "${v.sample}" --lang ${v.lang} --quality standard --voice "${v.voice}"`,
    `${cmt} ${t('conn.cmt_enh')}`, `${prefix}"${server}" speak "${v.sample}" --lang ${v.lang} --quality enhanced --voice "${v.voice}"`,
    `${cmt} ${t('conn.cmt_out')}`, `${prefix}"${server}" speak "${v.sample}" -l ${v.lang} -q enhanced -v "${v.voice}" -o "C:\\project\\voice\\01.wav"`
  ].join('\n');
  const blocks = [
    codeBlock(t('conn.mcp_json'), `{ "mcpServers": { "voiceclone": { "type": "http", "url": "${c.url}" } } }`),
    codeBlock(t('conn.claude_cmd'), `claude mcp add --transport http voiceclone ${c.url}`)
  ];
  if (hasServer) {
    blocks.push(
      codeBlock(t('conn.start'), `Start-Process "${server}" -WindowStyle Hidden`, t('conn.start_note')),
      codeBlock(t('conn.cli_ps'), cli('& ', '#'), t('conn.cli_note', v)),
      codeBlock(t('conn.cli_cmd'), cli('', 'REM')));
  }
  // Instructions completes : l'IA ou le skill n'a plus qu'a les lire.
  const doc = t('conn.doc', v);
  const aiCopy = el('button', { type: 'button', className: 'primary', textContent: t('conn.ai_copy') });
  aiCopy.onclick = async () => {
    try { await copyText(doc); say($('conn-status'), t('conn.copied_status')); } catch (e) { fail($('conn-status'), e); }
  };
  blocks.push(el('div', { className: 'codeblock' },
    el('div', { className: 'row head' }, el('strong', { textContent: t('conn.ai') }), aiCopy),
    el('details', {}, el('summary', { textContent: t('conn.ai_preview') }), el('pre', { textContent: doc }))));
  $('conn-blocks').replaceChildren(...blocks);
}
$('aud-open').onclick = () => invoke('open_audios_dir').catch((e) => fail($('status'), e));
async function loadConnection() {
  connInfo = await invoke('connection_info');
  $('aud-path').textContent = connInfo.audios;
  renderConnection();
}

// ---------------------------------------------------------------- parametres : mises a jour
function renderUpdate() {
  $('upd-version').textContent = t('upd.installed', { version: appVersion });
  const newer = !!updateInfo?.newer;
  $('upd-card').hidden = !newer;
  $('upd-badge').hidden = !newer;
  if (!newer) return;
  $('upd-badge').textContent = t('upd.badge', { latest: updateInfo.latest });
  $('upd-new').textContent = t('upd.available', { latest: updateInfo.latest, current: updateInfo.current });
  $('upd-notes').textContent = updateInfo.notes;
  const auto = !!updateInfo.asset?.sha256;
  $('upd-install').hidden = !auto;
  $('upd-noauto').hidden = auto;
  $('upd-size').textContent = updateInfo.asset ? t('upd.size', { name: updateInfo.asset.name, size: size(updateInfo.asset.size / 1048576) }) : '';
}
async function checkUpdate(silent) {
  const st = $('upd-status');
  if (!silent) say(st, t('upd.checking'));
  try {
    updateInfo = await invoke('check_update');
    lsSet('voiceclone-update-last', String(Date.now()));
    renderUpdate();
    if (!silent) say(st, updateInfo.newer ? '' : t('upd.uptodate', { version: updateInfo.current }));
  } catch (e) { if (!silent) fail(st, e); }
}
$('upd-check').onclick = () => checkUpdate(false);
$('upd-install').onclick = async () => {
  const st = $('upd-status'), bar = $('upd-progress');
  if (!(await ask(t('upd.install_confirm', { latest: updateInfo.latest })))) return;
  $('upd-install').disabled = true;
  bar.hidden = false;
  bar.value = 0;
  say(st, t('upd.downloading'));
  try {
    await invoke('install_update'); // le programme se ferme, est mis a jour puis se relance
    say(st, t('upd.restarting'));
  } catch (e) { $('upd-install').disabled = false; bar.hidden = true; fail(st, e); }
};
$('upd-open').onclick = () => invoke('open_release', { url: updateInfo.url }).catch((e) => fail($('upd-status'), e));
$('upd-auto').checked = lsGet('voiceclone-update-check') !== 'off';
$('upd-auto').onchange = () => lsSet('voiceclone-update-check', $('upd-auto').checked ? 'on' : 'off');
$('upd-badge').onclick = () => { show('settings'); selectTab('updates'); };

// ---------------------------------------------------------------- parametres : carte graphique
const gpuLabel = (d) => `${d.name} · ${d.description}${d.gpu && d.total_mb ? ` · ${t('gpu.gb', { n: (d.total_mb / 1024).toFixed(0) })}` : ''}`;
function fillTriOptions() {
  for (const id of ['gpu-fa', 'gpu-clamp']) {
    const keep = $(id).value;
    $(id).replaceChildren(...['auto', 'on', 'off'].map((value) => el('option', { value, textContent: t(`tri.${value}`) })));
    if (keep) $(id).value = keep;
  }
}

async function loadGpu() {
  const st = $('gpu-status');
  const g = await invoke('gpu');
  const byName = (n) => g.devices.find((d) => d.name === n);
  const best = byName(g.best);
  const choice = g.choice !== 'auto' && byName(g.choice) ? g.choice : 'auto';
  const target = choice === 'auto' ? best : byName(choice);

  $('gpu-summary').textContent = best
    ? t('gpu.summary', { best: gpuLabel(best), state: g.active ? t('gpu.loaded', { name: g.active }) : t('gpu.not_loaded') })
    : t('gpu.none');
  $('gpu-device').replaceChildren(
    el('option', { value: 'auto', textContent: t('gpu.auto', { name: best ? best.name : t('gpu.cpu_name') }) }),
    ...g.devices.map((d) => el('option', { value: d.name, textContent: gpuLabel(d) + (d.recommended ? ` · ${t('gpu.recommended')}` : '') })));
  $('gpu-device').value = choice;
  fillTriOptions();
  $('gpu-fa').value = g.fa;
  $('gpu-clamp').value = g.clamp;

  // Un choix manuel different du meilleur : on le dit et on propose de revenir au choix automatique.
  const hint = $('gpu-best-note');
  hint.hidden = !(choice !== 'auto' && best && target && target.name !== best.name);
  hint.textContent = hint.hidden ? '' : t('gpu.best_hint', { name: best.name, desc: best.description });
  $('gpu-nvidia').hidden = !g.nvidia_without_cuda;
  $('gpu-note').textContent = g.note ? t('gpu.fallback', { note: g.note }) : '';
  $('gpu-advanced').open ||= g.fa !== 'auto' || g.clamp !== 'auto';
  say(st, '');
}
async function saveGpu() {
  const st = $('gpu-status');
  try {
    await invoke('set_gpu', { device: $('gpu-device').value, fa: $('gpu-fa').value, clamp: $('gpu-clamp').value });
    await loadGpu();
    say(st, t('gpu.saved'));
  } catch (e) { fail(st, e); await loadGpu().catch(() => {}); }
}
for (const id of ['gpu-device', 'gpu-fa', 'gpu-clamp']) $(id).onchange = saveGpu;

// ---------------------------------------------------------------- parametres : composants et outils
let installing = null; // id du composant en cours d'installation
async function installComponent(c) {
  const st = $('comp-status');
  if (!(await ask(t(c.id === 'vulkan' ? 'comp.confirm_vulkan' : 'comp.confirm_cuda')))) return;
  installing = c.id;
  await loadComponents();
  try {
    const result = await invoke('install_component', { id: c.id });
    say(st, t(result === 'launched' ? 'comp.done_launched' : 'comp.done_installed'));
  } catch (e) { fail(st, e); }
  installing = null;
  await loadComponents();
}

async function loadComponents() {
  const list = await invoke('components');
  const open = (key, label) => { const b = el('button', { type: 'button', className: 'ghost', textContent: label }); b.onclick = () => invoke('open_official', { key }).catch((e) => fail($('comp-status'), e)); return b; };
  $('comp-list').replaceChildren(...list.map((c) => {
    const title = t(`comp.${c.id}`);
    const stateKey = { mp3: 'comp.mp3_state', vulkan: c.state === 'installed' ? 'comp.vulkan_ok' : 'comp.vulkan_missing',
      cuda: { not_applicable: 'comp.cuda_na', driver_missing: 'comp.cuda_driver', module_missing: 'comp.cuda_module', runtime_missing: 'comp.cuda_runtime', installed: 'comp.cuda_ok' }[c.state] }[c.id];
    const ok = c.state === 'installed' || c.state === 'builtin';
    const row = el('div', { className: 'row' });
    if (installing === c.id) {
      const bar = el('progress', { max: 100, value: 0 });
      bar.id = 'comp-bar';
      bar.setAttribute('aria-label', title);
      const cancel = el('button', { type: 'button', className: 'ghost', textContent: t('common.cancel') });
      cancel.onclick = () => invoke('cancel_component');
      row.append(bar, cancel);
    } else {
      if (c.can_install) {
        const b = el('button', { type: 'button', textContent: t(c.id === 'vulkan' ? 'comp.install_vulkan' : 'comp.install_cuda', { size: size(c.size_mb) }), disabled: installing !== null });
        b.onclick = () => installComponent(c);
        row.append(b);
      }
      if (c.id === 'vulkan' && c.state === 'missing') row.append(open('amd_drivers', t('comp.drivers_amd')), open('nvidia_drivers', t('comp.drivers_nvidia')), open('intel_drivers', t('comp.drivers_intel')));
      if (c.id === 'cuda' && c.state === 'driver_missing') row.append(open('nvidia_drivers', t('comp.drivers_nvidia')));
    }
    return el('li', { className: `comp${ok ? ' ok' : ''}` }, el('strong', { textContent: title }), el('p', { className: 'state', textContent: t(stateKey) }), row.children.length ? row : '');
  }));
}

// ---------------------------------------------------------------- parametres : modeles
const progress = {}; // fichier -> { done, total } pendant un telechargement

async function loadModels() {
  const st = $('models-status');
  const m = await invoke('models');
  $('models-hint').textContent = t('models.hint', { source: m.source.replace('https://', '').replace('/resolve/main/', '') });
  $('models-dir').textContent = t('mem.folder', { dir: m.dir });
  $('memory-state').textContent = m.loaded ? t('mem.loaded') : t('mem.none');
  $('unload').disabled = !m.loaded;
  $('model-list').replaceChildren(...m.models.map((x) => {
    const state = [x.downloaded ? t(x.active ? 'models.state_active' : 'models.state_installed') : t('models.state_missing'), x.loaded ? t('models.state_loaded') : ''].filter(Boolean).join(' · ');
    const row = el('li', { className: `model${x.active || (x.codec && x.downloaded) ? ' active' : ''}` },
      el('div', { className: 'grow' }, el('strong', { textContent: x.label }), el('span', { className: 'meta', textContent: `${size(x.size_mb)} · ${state}` })));
    const act = (label, fn, cls = '') => {
      const b = el('button', { type: 'button', className: cls, textContent: label });
      b.setAttribute('aria-label', t('models.action_aria', { action: label, label: x.label }));
      b.onclick = () => fn().catch((e) => fail(st, e));
      row.append(b);
    };
    if (x.downloading || progress[x.file]) {
      const p = progress[x.file] ?? { done: 0, total: 0 };
      const bar = el('progress', { max: 100, value: percent(p) });
      bar.dataset.file = x.file;
      bar.setAttribute('aria-label', t('models.progress_aria', { label: x.label }));
      const pct = el('span', { className: 'meta', textContent: `${percent(p)} %` });
      pct.dataset.pct = x.file;
      row.append(bar, pct);
      act(t('common.cancel'), () => invoke('cancel_download', { file: x.file }));
    } else if (!x.downloaded) {
      act(t('models.download', { size: size(x.size_mb) }), async () => {
        progress[x.file] = { done: 0, total: 0 };
        say(st, t('models.downloading', { label: x.label }));
        invoke('download_model', { file: x.file })
          .then(() => say(st, t('models.installed_done', { label: x.label })), (e) => fail(st, e))
          .finally(() => { delete progress[x.file]; loadModels().catch((e) => fail(st, e)); });
        await loadModels();
      });
    } else {
      if (!x.codec && !x.active) act(t('models.use'), async () => { await invoke('set_model', { file: x.file }); await loadModels(); say(st, t('models.use_done', { label: x.label })); });
      act(t('common.delete'), async () => {
        if (!(await ask(t('models.delete_confirm', { label: x.label, size: size(x.size_mb) })))) return;
        await invoke('delete_model', { file: x.file });
        await loadModels();
        say(st, t('models.deleted', { label: x.label }));
      }, 'ghost danger');
    }
    return row;
  }));
  $('unload').onclick = () => invoke('unload_model').then(() => Promise.all([loadModels(), loadGpu()])).then(() => say($('models-status'), t('mem.unloaded')), (e) => fail($('models-status'), e));
}

$('open-models').onclick = () => invoke('open_models_dir').catch((e) => fail($('models-status'), e));

function show(target) {
  page = target;
  const settings = page === 'settings';
  $('settings').hidden = !settings;
  $('studio').hidden = settings;
  $('nav-settings').setAttribute('aria-pressed', settings);
  $('nav-studio').setAttribute('aria-pressed', !settings);
  if (settings) loadSettings();
}
$('nav-studio').onclick = () => show('studio');
$('nav-settings').onclick = () => show('settings');

// ---------------------------------------------------------------- parametres : export et import des voix
async function renderVoicesTab() {
  const st = $('vx-status');
  try { const info = await invoke('voices_info'); $('vx-dir').textContent = t('vx.folder', { dir: info.dir, n: info.found }); } catch (e) { fail(st, e); }
  $('vx-list').replaceChildren(...profiles.map((p) => {
    const b = el('button', { type: 'button', textContent: t('vx.export'), disabled: !p.samples });
    b.setAttribute('aria-label', t('vx.export_aria', { name: p.name }));
    b.onclick = async () => {
      try { say(st, t('vx.exported', { path: await invoke('export_voice', { name: p.name }) })); await renderVoicesTab(); } catch (e) { fail(st, e); }
    };
    return el('li', { className: 'model' },
      el('div', { className: 'grow' }, el('strong', { textContent: p.name }), el('span', { className: 'meta', textContent: tp('voice.meta', { lang: langName(p.language), n: p.samples }) })), b);
  }));
}
$('vx-export-all').onclick = async () => {
  const st = $('vx-status');
  try { const names = await invoke('export_all_voices'); say(st, t('vx.exported_all', { n: names.length })); await renderVoicesTab(); } catch (e) { fail(st, e); }
};
$('vx-import').onclick = async () => {
  const st = $('vx-status');
  try {
    const r = await invoke('import_voices');
    const parts = [r.imported.length ? t('vx.imported', { names: r.imported.join(', ') }) : t('vx.nothing'), t('vx.existing', { n: r.existing })];
    if (r.errors.length) parts.push(t('vx.errors', { errors: r.errors.join(' · ') }));
    say(st, parts.join(' '));
    if (r.errors.length) st.classList.add('error');
    await loadProfiles();
    await renderVoicesTab();
  } catch (e) { fail(st, e); }
};
$('vx-open').onclick = () => invoke('open_voices_dir').catch((e) => fail($('vx-status'), e));

function loadSettings() {
  renderVoicesTab();
  renderUpdate();
  loadConnection().catch((e) => fail($('conn-status'), e));
  loadModels().catch((e) => fail($('models-status'), e));
  loadGpu().catch((e) => fail($('gpu-status'), e));
  loadComponents().catch((e) => fail($('comp-status'), e));
}

// Tout ce qui depend de la langue est reconstruit : textes statiques, listes, messages venant de Rust (libelles des modeles...).
let mcpInfo = null;
function renderMcp() {
  if (!mcpInfo) return;
  $('mcp-text').textContent = mcpInfo.error ? t('mcp.stopped', { error: mcpInfo.error }) : t('mcp.active', { url: mcpInfo.url });
  $('mcp').classList.toggle('error', !!mcpInfo.error);
}
async function renderAll() {
  applyStatic();
  fillLanguageOptions();
  renderLanguageChoices();
  renderThemeChoices();
  renderMcp();
  renderTabs();
  renderUpdate();
  renderConnection();
  if (recorder) $('rec-label').textContent = t('samples.stop_rec', { time: '0:00' });
  await loadProfiles();
  await loadGenerations();
  if (page === 'settings') loadSettings();
}

// ---------------------------------------------------------------- demarrage
(async () => {
  try {
    // Langue : celle enregistree par le coeur ; sinon celle du systeme, qu'on enregistre pour la version serveur aussi.
    const saved = await invoke('get_lang');
    if (saved) setLanguage(saved);
    else await invoke('set_lang', { lang: LANG });
    appVersion = await invoke('app_version');

    await listen('model-progress', ({ payload: p }) => {
      if (!progress[p.file]) return; // telechargement deja termine ou annule
      progress[p.file] = p;
      const bar = document.querySelector(`progress[data-file="${p.file}"]`), pct = document.querySelector(`[data-pct="${p.file}"]`);
      if (bar) bar.value = percent(p);
      if (pct) pct.textContent = t('models.progress', { pct: percent(p), done: size(Math.round(p.done / 1048576)), total: size(Math.round(p.total / 1048576)) });
    });
    await listen('update-progress', ({ payload: p }) => {
      $('upd-progress').value = percent(p);
      say($('upd-status'), t('upd.progress', { pct: percent(p), done: size(Math.round(p.done / 1048576)), total: size(Math.round(p.total / 1048576)) }));
    });
    await listen('component-progress', ({ payload: p }) => {
      const bar = $('comp-bar');
      if (bar) bar.value = percent(p);
      const key = { manifest: 'comp.stage_manifest', verify: 'comp.stage_verify', extract: 'comp.stage_extract', launch: 'comp.stage_launch' }[p.stage];
      say($('comp-status'), key ? t(key) : t('comp.stage_download', { pct: percent(p) }));
    });
    // Resultat de l'installation precedente (ecrit par le script de mise a jour).
    const upd = await invoke('update_log');
    if (upd) {
      show('settings');
      selectTab('updates');
      if (upd === 'ok') say($('upd-status'), t('upd.done', { version: appVersion })); else fail($('upd-status'), upd);
    }
    mcpInfo = await invoke('mcp_info');
    await listen('changed', () => loadGenerations().catch((e) => fail($('status'), e)));
    await renderAll();

    // Detection automatique d'une nouvelle version : une fois par jour au plus, sauf si l'utilisateur l'a desactivee.
    const last = Number(lsGet('voiceclone-update-last') ?? 0);
    if (lsGet('voiceclone-update-check') !== 'off' && Date.now() - last > 24 * 3600 * 1000) checkUpdate(true);
  } catch (e) { fail($('status'), e); }
})();
