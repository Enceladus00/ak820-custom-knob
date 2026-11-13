'use strict';

// Settings UI. State lives in `state.config` (the same shape as config.toml,
// as JSON); every edit goes through it and the Rust side validates and saves.
// All visible text comes from t() (i18n.js + locales/).

// ---------------------------------------------------------------- backend

let invoke = null;
if (window.__TAURI__) invoke = window.__TAURI__.core.invoke;

// For working on the UI in a normal browser: call startPreview(mockInvoke).
window.startPreview = (mock) => { invoke = mock; init(); };

// ---------------------------------------------------------------- constants

const PAGES = [
  { id: 'modes', icon: 'dial' },
  { id: 'apps', icon: 'apps' },
  { id: 'remap', icon: 'remap' },
  { id: 'macros', icon: 'macro' },
  { id: 'snippets', icon: 'text' },
  { id: 'controls', icon: 'pointer' },
  { id: 'lighting', icon: 'light' },
  { id: 'effects', icon: 'spark' },
  { id: 'general', icon: 'gear' },
];

// Effects a layout can use. Labels: lighting.effects.<value>.
// `color`: the effect can use a chosen colour. `speed`: it has a speed.
const EFFECTS = [
  { value: 1, color: true, speed: false },
  { value: 2, color: true, speed: true },
  { value: 3, color: true, speed: true },
  { value: 4, color: true, speed: true },
  { value: 5, color: true, speed: true },
  { value: 6, color: false, speed: true },
  { value: 7, color: true, speed: true },
  { value: 8, color: false, speed: true },
  { value: 9, color: true, speed: true },
  { value: 10, color: true, speed: true },
  { value: 11, color: true, speed: true },
  { value: 12, color: true, speed: true },
  { value: 13, color: true, speed: true },
  { value: 14, color: true, speed: true },
  { value: 15, color: true, speed: true },
  { value: 16, color: true, speed: true },
  { value: 17, color: true, speed: true },
  { value: 18, color: true, speed: true },
  { value: 19, color: true, speed: true },
];

// Starting points for the click-hint look. Everything stays editable.
const HINT_PRESETS = {
  midnight: { background: '#171B24', text: '#FFFFFF', typed: '#7AA7FF', radius: 7, opacity: 94 },
  paper: { background: '#FFFFFF', text: '#1C1F24', typed: '#2F6FEB', radius: 7, opacity: 96 },
  highlighter: { background: '#FFD84D', text: '#1A1A1A', typed: '#A06A00', radius: 4, opacity: 100 },
  ocean: { background: '#0E4C75', text: '#EAF6FF', typed: '#6FD3FF', radius: 9, opacity: 94 },
  rose: { background: '#3B1530', text: '#FFEAF4', typed: '#FF7AB8', radius: 9, opacity: 94 },
};

const effectInfo = (value) => EFFECTS.find((e) => e.value === value) ?? EFFECTS[0];

const GESTURES = [
  { key: 'rotate', kind: 'rotate' },
  { key: 'click', kind: 'press' },
  { key: 'long_press', kind: 'press' },
  { key: 'double_click', kind: 'press' },
];

const LAYERS = ['', 'ctrl', 'shift', 'alt', 'win'];

// [id, defaults for a newly picked action]. Labels: actions.<kind>.<id>.
const ACTIONS = {
  rotate: [
    ['volume', { step: 2, accelerate: true }],
    ['app_volume', { step: 5, accelerate: true }],
    ['mic_volume', { step: 5, accelerate: true }],
    ['audio_output', {}],
    ['scroll', { lines: 1, horizontal: false, accelerate: true }],
    ['zoom', {}],
    ['keys', { cw: 'Right', ccw: 'Left' }],
    ['passthrough', {}],
    ['none', {}],
  ],
  press: [
    ['mute', {}],
    ['mic_mute', {}],
    ['audio_output', {}],
    ['open', { target: '', args: '' }],
    ['cycle_mode', {}],
    ['keys', { keys: 'Space' }],
    ['mouse_click', { button: 'middle' }],
    ['run_macro', { name: '' }],
    ['toggle_mouse_mode', {}],
    ['click_hints', {}],
    ['passthrough', {}],
    ['none', {}],
  ],
};

const SPECIAL_KEYS = ['MediaPlayPause', 'MediaNext', 'MediaPrev', 'MediaStop', 'VolumeMute',
  'BrowserBack', 'BrowserForward', 'BrowserRefresh', 'PrintScreen'];

const THEMES = ['system', 'light', 'dark'];

const ICONS = {
  text: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7V5h12v2M10 5v14M7 19h6"/><path d="M15 13h5M17.5 13v6"/></svg>',
  dial: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M5.6 18.4A9 9 0 1 1 18.4 18.4"/><circle cx="12" cy="12" r="4"/><path d="M14.5 9.5 17 7"/></svg>',
  apps: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7.5" height="7.5" rx="2"/><rect x="13.5" y="3" width="7.5" height="7.5" rx="2"/><rect x="3" y="13.5" width="7.5" height="7.5" rx="2"/><rect x="13.5" y="13.5" width="7.5" height="7.5" rx="2"/></svg>',
  gear: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M2 12h3M19 12h3M4.9 19.1 7 17M17 7l2.1-2.1"/></svg>',
  light: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18h6M10 21h4"/><path d="M12 3a6 6 0 0 0-3.8 10.6c.7.6 1.1 1.3 1.2 2.4h5.2c.1-1.1.5-1.8 1.2-2.4A6 6 0 0 0 12 3Z"/></svg>',
  macro: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 10h.01M11 10h.01M15 10h.01M8 14h8"/></svg>',
  pointer: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 3l6.5 17 2.4-6.9L21 10.7 5 3Z"/></svg>',
  remap: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="8" width="7" height="8" rx="1.5"/><rect x="15" y="8" width="7" height="8" rx="1.5"/><path d="M10.5 12h3.5M12.5 10l2 2-2 2"/></svg>',
  spark: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 3.5 11.6 8.4 16.5 10 11.6 11.6 10 16.5 8.4 11.6 3.5 10 8.4 8.4Z"/><path d="M18 14v6M15 17h6"/></svg>',
  plus: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>',
};

// ---------------------------------------------------------------- state

const state = {
  config: null,
  savedJson: '',
  path: '',
  loadError: null,
  errors: [],
  page: 'modes',
  modeIndex: 0,
  layer: '',
  layoutIndex: 0,
  effectIndex: 0,
  macroIndex: 0,
  recording: null, // { macro, last } while a macro is being recorded
  applying: false,
  autostart: null, // { available, enabled }; null until known
  daemon: null, // { running, available }; null until known
  version: '',
  picker: null, // index of the profile, remap or macro the app picker adds to
  pickerFor: 'profiles', // 'profiles', 'remaps' or 'macros'
  apps: null,
  appFilter: '',
  saving: false,
};

// ---------------------------------------------------------------- helpers

const $ = (selector) => document.querySelector(selector);
const esc = (value) => String(value ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const clone = (value) => JSON.parse(JSON.stringify(value));
const icon = (name) => ICONS[name] || '';

function getPath(obj, path) {
  return path.split('.').reduce((o, k) => (o == null ? undefined : o[k]), obj);
}

function setPath(obj, path, value) {
  const keys = path.split('.');
  let o = obj;
  for (const k of keys.slice(0, -1)) {
    if (o[k] == null) o[k] = {};
    o = o[k];
  }
  const last = keys[keys.length - 1];
  if (value === undefined) delete o[last];
  else o[last] = value;
}

// The Rust side names the lists after the TOML tables ([[mode]], [[profile]]);
// the UI uses plural names. Convert at the boundary.
function fromRust(raw) {
  const { mode, profile, macro, remap, ...rest } = raw;
  const config = { ...rest, modes: mode, profiles: profile, macros: macro ?? [], remaps: remap ?? [] };
  for (const p of config.profiles) {
    if (typeof p.match.exe === 'string') p.match.exe = [p.match.exe];
  }
  for (const r of config.remaps) r.apps ??= [];
  for (const m of config.macros) m.apps ??= [];
  for (const e of config.effects ?? []) e.when ??= [];
  return config;
}

function toRust(config) {
  const { modes, profiles, macros, remaps, ...rest } = config;
  return { ...rest, mode: modes, profile: profiles, macro: macros, remap: remaps };
}

const isDirty = () => JSON.stringify(state.config) !== state.savedJson;
const currentMode = () => state.config.modes[state.modeIndex];
const modeName = (id) => state.config.modes.find((m) => m.id === id)?.name ?? id;

function uniqueId(base) {
  const ids = new Set(state.config.modes.map((m) => m.id));
  const stem = base.replace(/-\d+$/, '') || 'mode';
  for (let n = 2; ; n++) {
    const id = `${stem}-${n}`;
    if (!ids.has(id)) return id;
  }
}

function modeUsage(id) {
  const names = state.config.profiles.filter((p) => p.modes.includes(id)).map((p) => p.name);
  return names.length ? t('modes.usedIn', { names: names.join(', ') }) : t('modes.unused');
}

function layerCount(mode, layer) {
  const bindings = layer ? mode[layer] : mode;
  if (!bindings) return 0;
  return GESTURES.filter((g) => bindings[g.key]).length;
}

// ---------------------------------------------------------------- appearance

function applyAppearance() {
  const ui = state.config?.ui ?? { theme: 'system', language: 'auto' };
  setLanguage(ui.language);

  const root = document.documentElement;
  if (ui.theme === 'system') delete root.dataset.theme;
  else root.dataset.theme = ui.theme;
  // Title bar colours too (ignored outside the app).
  window.__TAURI__?.window?.getCurrentWindow?.().setTheme(ui.theme === 'system' ? null : ui.theme).catch(() => {});

  document.querySelectorAll('[data-i18n]').forEach((el) => { el.innerHTML = t(el.dataset.i18n); });
  document.querySelectorAll('[data-i18n-title]').forEach((el) => { el.title = tPlain(el.dataset.i18nTitle); });
  document.title = `${tPlain('app.name')} – ${tPlain('app.subtitle')}`;
}

// ---------------------------------------------------------------- rendering

function render() {
  applyAppearance();
  $('#nav').innerHTML = PAGES.map((p) => `
    <button class="nav-item ${p.id === state.page ? 'active' : ''}" data-cmd="nav" data-page="${p.id}">
      ${icon(p.icon)}<span>${t(`pages.${p.id}.label`)}</span>
    </button>`).join('');
  $('#title').innerHTML = t(`pages.${state.page}.title`);
  $('#subtitle').innerHTML = t(`pages.${state.page}.sub`);

  const main = $('#main');
  const scroll = main.scrollTop;
  const pages = { modes: modesPage, apps: appsPage, remap: remapPage, macros: macrosPage, snippets: snippetsPage, controls: controlsPage, lighting: lightingPage, effects: effectsPage, general: generalPage };
  main.innerHTML = pages[state.page]();
  main.scrollTop = scroll;
  renderPicker();
  updateChrome();
}

function updateChrome() {
  const dirty = isDirty();
  const problems = state.errors.length;
  const status = $('#status');
  status.className = `status ${problems ? 'error' : dirty ? 'dirty' : 'saved'}`;
  status.innerHTML = state.saving ? t('status.saving')
    : problems ? t('status.problems', { count: problems })
    : dirty ? t('status.unsaved') : t('status.saved');
  $('#save').disabled = !dirty || problems > 0 || state.saving;
  $('#discard').disabled = !dirty || state.saving;

  const banners = [];
  if (state.loadError) {
    banners.push(`<div class="banner warn">${t('banners.loadError', { error: state.loadError })}</div>`);
  }
  if (state.daemon && !state.daemon.running) {
    banners.push(`<div class="banner warn banner-action"><span>${t('banners.notRunning')}</span>${state.daemon.available ? `<button class="btn small" data-cmd="start-daemon">${t('banners.start')}</button>` : ''}</div>`);
  }
  if (problems) {
    banners.push(`<div class="banner error">${t('banners.problems')}
      <ul>${state.errors.map((e) => `<li>${esc(e)}</li>`).join('')}</ul></div>`);
  }
  $('#banners').innerHTML = banners.join('');
}

// ---------- modes page

function modesPage() {
  const modes = state.config.modes;
  state.modeIndex = Math.min(state.modeIndex, Math.max(0, modes.length - 1));
  const list = modes.map((m, i) => `
    <button class="mode-item ${i === state.modeIndex ? 'active' : ''}" data-cmd="select-mode" data-index="${i}">
      <span class="mode-name">${esc(m.name) || t('modes.unnamed')}</span>
      <span class="mode-meta">${modeUsage(m.id)}</span>
    </button>`).join('');
  const mode = currentMode();
  return `<div class="modes">
    <div class="mode-list">
      ${list}
      <button class="btn ghost" data-cmd="add-mode">${icon('plus')} ${t('modes.newMode')}</button>
    </div>
    ${mode ? modeEditor(mode, state.modeIndex) : `<div class="card muted">${t('modes.empty')}</div>`}
  </div>`;
}

function modeEditor(mode, i) {
  const layer = state.layer;
  const tabs = LAYERS.map((key) => {
    const n = key ? layerCount(mode, key) : 0;
    return `<button class="tab ${layer === key ? 'active' : ''}" data-cmd="layer" data-layer="${key}">
      ${t(`layers.${key || 'none'}`)}${n ? `<span class="count">${n}</span>` : ''}</button>`;
  }).join('');
  const hint = layer ? t('layers.hint', { key: tPlain(`layers.${layer}`) }) : t('layers.hintNone');
  return `<section class="card editor">
    <div class="editor-head">
      <label class="field grow"><span>${t('modes.name')}</span>
        <input type="text" data-bind="modes.${i}.name" value="${esc(mode.name)}" maxlength="40" spellcheck="false">
      </label>
      <div class="head-actions">
        <button class="btn" data-cmd="duplicate-mode">${t('buttons.duplicate')}</button>
        <button class="btn danger" data-cmd="delete-mode">${t('buttons.delete')}</button>
      </div>
    </div>
    <div class="tabs" role="tablist">${tabs}</div>
    <p class="hint">${hint}</p>
    <div class="gestures">${GESTURES.map((g) => gestureRow(i, g, layer)).join('')}</div>
  </section>`;
}

function gestureRow(i, gesture, layer) {
  const path = layer ? `modes.${i}.${layer}.${gesture.key}` : `modes.${i}.${gesture.key}`;
  const action = getPath(state.config, path);
  const current = action?.action ?? '';
  const label = t(`gestures.${gesture.key}.label`);
  const options = [`<option value="">${t(layer ? 'actions.inherit' : 'actions.unset')}</option>`]
    .concat(ACTIONS[gesture.kind].map(([id]) =>
      `<option value="${id}" ${id === current ? 'selected' : ''}>${t(`actions.${gesture.kind}.${id}`)}</option>`))
    .join('');
  return `<div class="gesture ${current ? '' : 'unset'}">
    <div class="gesture-label"><strong>${label}</strong><small>${t(`gestures.${gesture.key}.hint`)}</small></div>
    <div class="gesture-body">
      <select data-cmd="set-action" data-path="${path}" data-kind="${gesture.kind}" aria-label="${esc(tPlain(`gestures.${gesture.key}.label`))}">${options}</select>
      ${action ? paramsHtml(action, path, gesture.kind) : ''}
    </div>
  </div>`;
}

function paramsHtml(action, path, kind) {
  const parts = [];
  if ('step' in action) parts.push(numberParam(`${path}.step`, action.step, 1, 25, t('params.stepUnit')));
  if ('lines' in action) parts.push(numberParam(`${path}.lines`, action.lines, 1, 20, t('params.linesUnit')));
  if ('horizontal' in action) parts.push(checkParam(`${path}.horizontal`, action.horizontal, t('params.sideways')));
  if ('accelerate' in action) parts.push(checkParam(`${path}.accelerate`, action.accelerate, t('params.accelerate')));
  if (action.action === 'keys') {
    if (kind === 'rotate') {
      parts.push(keyParam(`${path}.cw`, action.cw, t('params.clockwise')));
      parts.push(keyParam(`${path}.ccw`, action.ccw, t('params.counterClockwise')));
    } else {
      parts.push(keyParam(`${path}.keys`, action.keys, t('params.keys')));
    }
  }
  if (action.action === 'run_macro') {
    const macros = state.config.macros;
    parts.push(macros.length
      ? `<label class="param"><select data-bind="${path}.name" aria-label="${esc(tPlain('macros.name'))}">
        ${macros.map((m) => `<option value="${esc(m.name)}" ${m.name === action.name ? 'selected' : ''}>${esc(m.name)}</option>`).join('')}
      </select></label>`
      : `<span class="muted">${t('macros.noneYet')}</span>`);
  }
  if (action.action === 'open') {
    parts.push(`<label class="field grow open-target"><span>${t('params.target')}</span>
      <input type="text" data-bind="${path}.target" value="${esc(action.target)}" placeholder="${esc(tPlain('params.targetExample'))}" spellcheck="false"></label>`);
    parts.push(`<label class="field open-args"><span>${t('params.args')}</span>
      <input type="text" data-bind="${path}.args" value="${esc(action.args ?? '')}" spellcheck="false"></label>`);
  }
  if (action.action === 'mouse_click') {
    parts.push(`<label class="param"><select data-bind="${path}.button">
      ${['left', 'middle', 'right'].map((b) => `<option value="${b}" ${b === action.button ? 'selected' : ''}>${t(`params.mouse.${b}`)}</option>`).join('')}
    </select></label>`);
  }
  return parts.length ? `<div class="params">${parts.join('')}</div>` : '';
}

const numberParam = (path, value, min, max, unit) => `<label class="param">
  <input type="number" min="${min}" max="${max}" value="${esc(value)}" data-bind="${path}" data-type="int"><span>${unit}</span></label>`;

const checkParam = (path, value, label) => `<label class="param check">
  <input type="checkbox" data-bind="${path}" data-type="bool" ${value ? 'checked' : ''}><span>${label}</span></label>`;

const keyParam = (path, value, label) => `<div class="param keyfield">
  <span class="param-label">${label}</span>
  <input class="keycap" readonly data-keycap="${path}" value="${esc(value)}" title="${esc(tPlain('params.keycapTitle'))}">
  <select class="special" data-special="${path}" aria-label="${esc(tPlain('params.specialKey'))}">
    <option value="">${t('params.specialKey')}</option>
    ${SPECIAL_KEYS.map((k) => `<option value="${k}">${t(`specialKeys.${k}`)}</option>`).join('')}
  </select>
</div>`;

// ---------- apps page

function appsPage() {
  const profiles = state.config.profiles;
  return `<p class="lead">${t('apps.lead')}</p>
    ${profiles.map((p, i) => profileCard(p, i, profiles.length)).join('')}
    <button class="btn ghost" data-cmd="add-profile">${icon('plus')} ${t('apps.addProfile')}</button>`;
}

function profileCard(profile, i, count) {
  const exes = profile.match.exe;
  const everyApp = exes.includes('*');
  const remove = esc(tPlain('apps.remove'));
  const apps = everyApp
    ? `<span class="chip all">${t('apps.everyApp')}</span>`
    : exes.length
      ? exes.map((exe, k) => `<span class="chip">${esc(exe)}<button data-cmd="remove-app" data-profile="${i}" data-index="${k}" title="${remove}">×</button></span>`).join('')
      : `<span class="muted">${t('apps.noApps')}</span>`;
  const modes = profile.modes.map((id, k) => `<span class="chip mode">${k + 1}. ${esc(modeName(id))}
      <button data-cmd="mode-move" data-profile="${i}" data-index="${k}" data-delta="-1" title="${esc(tPlain('apps.modeEarlier'))}" ${k === 0 ? 'disabled' : ''}>‹</button>
      <button data-cmd="mode-move" data-profile="${i}" data-index="${k}" data-delta="1" title="${esc(tPlain('apps.modeLater'))}" ${k === profile.modes.length - 1 ? 'disabled' : ''}>›</button>
      <button data-cmd="remove-profile-mode" data-profile="${i}" data-index="${k}" title="${remove}">×</button>
    </span>`).join('');
  const addable = state.config.modes.filter((m) => !profile.modes.includes(m.id));
  const addMode = addable.length
    ? `<select data-cmd="add-profile-mode" data-profile="${i}" aria-label="${esc(tPlain('apps.addMode'))}">
        <option value="">${t('apps.addMode')}</option>
        ${addable.map((m) => `<option value="${esc(m.id)}">${esc(m.name)}</option>`).join('')}
      </select>`
    : '';
  const shadowing = everyApp && i < count - 1 ? `<div class="banner warn">${t('apps.shadowing')}</div>` : '';
  const noApps = !everyApp && !exes.length ? `<div class="banner warn">${t('apps.noAppsWarning')}</div>` : '';

  return `<section class="card profile">
    <div class="profile-head">
      <span class="order" title="${esc(tPlain('apps.order'))}">${i + 1}</span>
      <input type="text" class="title-input" data-bind="profiles.${i}.name" value="${esc(profile.name)}" aria-label="${esc(tPlain('apps.name'))}" spellcheck="false">
      <div class="head-actions">
        <button class="icon-btn" data-cmd="profile-move" data-index="${i}" data-delta="-1" title="${esc(tPlain('apps.earlier'))}" ${i === 0 ? 'disabled' : ''}>↑</button>
        <button class="icon-btn" data-cmd="profile-move" data-index="${i}" data-delta="1" title="${esc(tPlain('apps.later'))}" ${i === count - 1 ? 'disabled' : ''}>↓</button>
        <button class="icon-btn danger" data-cmd="delete-profile" data-index="${i}" title="${esc(tPlain('apps.delete'))}" ${count === 1 ? 'disabled' : ''}>✕</button>
      </div>
    </div>
    <div class="profile-row">
      <span class="row-label">${t('apps.apps')}</span>
      <div class="chips">${apps}${everyApp ? '' : `<button class="btn small" data-cmd="open-picker" data-profile="${i}">${icon('plus')} ${t('apps.addApp')}</button>`}</div>
      <label class="param check"><input type="checkbox" data-cmd="toggle-every-app" data-profile="${i}" ${everyApp ? 'checked' : ''}><span>${t('apps.everyApp')}</span></label>
    </div>
    <div class="profile-row">
      <span class="row-label">${t('apps.modes')}</span>
      <div class="chips">${modes || `<span class="muted">${t('apps.noModes')}</span>`}${addMode}</div>
      <span></span>
    </div>
    <div class="profile-row">
      <span></span>
      <label class="param check"><input type="checkbox" data-bind="profiles.${i}.remember_last_mode" data-type="bool" ${profile.remember_last_mode ? 'checked' : ''}>
        <span>${t('apps.remember')}</span></label>
      <span></span>
    </div>
    ${shadowing}${noApps}
  </section>`;
}

// ---------- remap page

// Free keys offered first for a new remap.
const REMAP_CANDIDATES = ['CapsLock', 'F13', 'F14', 'F15', 'F16', 'F17', 'F18', 'F19', 'F20', 'ScrollLock', 'Pause', 'Insert'];

function remapPage() {
  const remaps = state.config.remaps;
  return `<p class="lead">${t('remap.lead')}</p>
    ${remaps.length
      ? `<section class="card remap-list">${remaps.map(remapRow).join('')}</section>`
      : `<div class="card muted">${t('remap.empty')}</div>`}
    <button class="btn ghost" data-cmd="add-remap">${icon('plus')} ${t('remap.add')}</button>
    <p class="hint">${t('remap.note')}</p>`;
}

function remapRow(rule, i) {
  const remove = esc(tPlain('apps.remove'));
  const apps = rule.apps.length
    ? rule.apps.map((exe, k) => `<span class="chip">${esc(exe)}<button data-cmd="remap-remove-app" data-index="${i}" data-app="${k}" title="${remove}">×</button></span>`).join('')
    : `<span class="chip all">${t('apps.everyApp')}</span>`;
  const special = SPECIAL_KEYS.map((k) => `<option value="${k}">${t(`specialKeys.${k}`)}</option>`).join('');
  return `<div class="remap-row">
    <div class="remap-key">
      <span class="param-label">${t('remap.from')}</span>
      <input class="keycap" readonly data-keycap="remaps.${i}.from" data-keymode="source" value="${esc(rule.from)}" title="${esc(tPlain('remap.fromTitle'))}" aria-label="${esc(tPlain('remap.from'))}">
    </div>
    <span class="remap-arrow" aria-hidden="true">→</span>
    <div class="remap-key">
      <span class="param-label">${t('remap.to')}</span>
      <div class="remap-to">
        <input class="keycap" readonly data-keycap="remaps.${i}.to" data-keymode="target" value="${esc(rule.to)}" title="${esc(tPlain('remap.toTitle'))}" aria-label="${esc(tPlain('remap.to'))}">
        <select class="special" data-special="remaps.${i}.to" aria-label="${esc(tPlain('params.specialKey'))}">
          <option value="">${t('params.specialKey')}</option>${special}
        </select>
      </div>
    </div>
    <div class="remap-apps">
      <span class="param-label">${t('remap.where')}</span>
      <div class="chips">${apps}<button class="btn small" data-cmd="open-remap-picker" data-index="${i}">${icon('plus')} ${rule.apps.length ? t('apps.addApp') : t('remap.onlyIn')}</button></div>
    </div>
    <button class="icon-btn danger" data-cmd="delete-remap" data-index="${i}" title="${esc(tPlain('remap.delete'))}">✕</button>
  </div>`;
}

// The exe list the open app picker adds to.
function pickerList() {
  if (state.picker == null) return null;
  if (state.pickerFor === 'remaps') return state.config.remaps[state.picker]?.apps;
  if (state.pickerFor === 'macros') return state.config.macros[state.picker]?.apps;
  if (state.pickerFor === 'snippet') return snip.draft?.apps;
  if (state.pickerFor === 'snippetExclude') return snip.store?.exclude;
  return state.config.profiles[state.picker]?.match.exe;
}

function renderPicker() {
  const root = $('#modal-root');
  if (state.picker == null) {
    root.innerHTML = '';
    return;
  }
  if (!root.querySelector('.overlay')) {
    root.innerHTML = `<div class="overlay" data-cmd="close-picker">
      <div class="modal" role="dialog" aria-label="${esc(tPlain('apps.picker.title'))}">
        <h2>${t('apps.picker.title')}</h2>
        <p class="hint">${t('apps.picker.hint')}</p>
        <input type="search" id="app-filter" placeholder="${esc(tPlain('apps.picker.search'))}" value="${esc(state.appFilter)}">
        <div class="app-list" id="app-list"></div>
        <div class="modal-foot">
          <input type="text" id="app-manual" placeholder="${esc(tPlain('apps.picker.manual'))}" spellcheck="false">
          <button class="btn" data-cmd="add-manual-app">${t('buttons.add')}</button>
          <span class="spacer"></span>
          <button class="btn ghost" data-cmd="close-picker">${t('buttons.close')}</button>
        </div>
      </div>
    </div>`;
    $('#app-filter').focus();
  }
  renderAppList();
}

function renderAppList() {
  const list = $('#app-list');
  if (!list) return;
  if (state.apps == null) {
    list.innerHTML = `<div class="muted pad">${t('apps.picker.loading')}</div>`;
    return;
  }
  const filter = state.appFilter.toLowerCase();
  const taken = new Set(pickerList()?.map((e) => e.toLowerCase()) ?? []);
  const apps = state.apps.filter((a) => !taken.has(a.exe.toLowerCase())
    && (!filter || a.exe.toLowerCase().includes(filter) || a.title.toLowerCase().includes(filter)));
  list.innerHTML = apps.length
    ? apps.map((a) => `<button class="app-row" data-cmd="pick-app" data-exe="${esc(a.exe)}"><strong>${esc(a.exe)}</strong><span>${esc(a.title)}</span></button>`).join('')
    : `<div class="muted pad">${t('apps.picker.none')}</div>`;
}

// ---------- lighting page

const currentLayout = () => state.config.lighting[state.layoutIndex];

function layoutSwatch(layout) {
  const paint = layout.colorful || !effectInfo(layout.effect).color
    ? 'conic-gradient(#ff4d4d, #ffd24d, #4dff88, #4dc3ff, #b14dff, #ff4d4d)'
    : esc(layout.color);
  return `<span class="swatch" style="background:${paint}"></span>`;
}

function lightingPage() {
  const layouts = state.config.lighting;
  state.layoutIndex = Math.min(state.layoutIndex, Math.max(0, layouts.length - 1));
  const list = layouts.map((l, i) => `
    <button class="mode-item ${i === state.layoutIndex ? 'active' : ''}" data-cmd="select-layout" data-index="${i}">
      <span class="mode-name">${layoutSwatch(l)}${esc(l.name) || t('modes.unnamed')}</span>
      <span class="mode-meta">${t(`lighting.effects.${l.effect}`)}</span>
    </button>`).join('');
  const layout = currentLayout();
  return `<div class="modes">
    <div class="mode-list">
      ${list}
      <button class="btn ghost" data-cmd="add-layout">${icon('plus')} ${t('lighting.newLayout')}</button>
    </div>
    ${layout ? layoutEditor(layout, state.layoutIndex) : `<div class="card muted">${t('lighting.empty')}</div>`}
  </div>`;
}

const levelField = (i, key, value) => `<div class="range-field">
  <div><strong>${t(`lighting.${key}`)}</strong></div>
  <input type="range" min="1" max="5" step="1" value="${esc(value)}" data-bind="lighting.${i}.${key}" data-type="int" data-plain="1" aria-label="${esc(tPlain(`lighting.${key}`))}">
  <output>${esc(value)}</output>
</div>`;

function layoutEditor(layout, i) {
  const info = effectInfo(layout.effect);
  const effects = EFFECTS.map((e) => `<option value="${e.value}" ${e.value === layout.effect ? 'selected' : ''}>${t(`lighting.effects.${e.value}`)}</option>`).join('');
  const colorRow = info.color ? `<div class="setting-row">
      <div><strong>${t('lighting.color')}</strong></div>
      <div class="color-controls">
        <label class="param check"><input type="checkbox" data-cmd="layout-colorful" ${layout.colorful ? 'checked' : ''}><span>${t('lighting.colorful')}</span></label>
        ${layout.colorful ? '' : `<input type="color" data-bind="lighting.${i}.color" value="${esc(layout.color.toLowerCase())}" aria-label="${esc(tPlain('lighting.color'))}">`}
      </div>
    </div>` : '';
  return `<section class="card editor">
    <div class="editor-head">
      <label class="field grow"><span>${t('lighting.name')}</span>
        <input type="text" data-bind="lighting.${i}.name" value="${esc(layout.name)}" maxlength="40" spellcheck="false">
      </label>
      <div class="head-actions">
        <button class="btn" data-cmd="duplicate-layout">${t('buttons.duplicate')}</button>
        <button class="btn danger" data-cmd="delete-layout">${t('buttons.delete')}</button>
      </div>
    </div>
    <div class="setting-row">
      <div><strong>${t('lighting.effect')}</strong></div>
      <select data-cmd="layout-effect" aria-label="${esc(tPlain('lighting.effect'))}">${effects}</select>
    </div>
    ${colorRow}
    ${levelField(i, 'brightness', layout.brightness)}
    ${info.speed ? levelField(i, 'speed', layout.speed) : ''}
    <div class="apply-row">
      <button class="btn primary" data-cmd="apply-layout" ${state.applying ? 'disabled' : ''}>${t('lighting.apply')}</button>
      <small class="hint">${t('lighting.applyHint')}</small>
    </div>
  </section>`;
}

async function applyLayout() {
  const layout = currentLayout();
  if (!layout || state.applying) return;
  state.applying = true;
  render();
  try {
    await invoke('apply_lighting', { layout });
    toast(tPlain('lighting.applied', { name: layout.name }));
  } catch (e) {
    const known = ['no_receiver', 'no_reply'].includes(e);
    toast(known ? tPlain(`lighting.errors.${e}`) : tPlain('lighting.errors.other', { error: e }), true);
  }
  state.applying = false;
  render();
}

// ---------- effects page
//
// Effects for the patched (LIVE v2) keyboard firmware. The shape, the checks
// and the pattern formulas are in docs/effects-format.md; the checks run on
// the Rust side like the rest of the config. The preview draws the formulas
// in the browser. Nothing here talks to the keyboard yet.

const PATTERNS = ['solid', 'pulse', 'wave', 'flicker', 'sparkle'];
const EFFECT_SLOTS = 8;
const EFFECT_MAX_COLORS = 4;

// knob_rgb::KEYS (crates/knob-rgb/src/lib.rs): name, row, column.
const KEYBOARD_KEYS = [
  ['Esc', 0, 0], ['F1', 0, 1], ['F2', 0, 2], ['F3', 0, 3], ['F4', 0, 4],
  ['F5', 0, 5], ['F6', 0, 6], ['F7', 0, 7], ['F8', 0, 8], ['F9', 0, 9],
  ['F10', 0, 10], ['F11', 0, 11], ['F12', 0, 12], ['Delete', 0, 13],
  ['`', 1, 0], ['1', 1, 1], ['2', 1, 2], ['3', 1, 3], ['4', 1, 4],
  ['5', 1, 5], ['6', 1, 6], ['7', 1, 7], ['8', 1, 8], ['9', 1, 9],
  ['0', 1, 10], ['-', 1, 11], ['=', 1, 12], ['Backspace', 1, 13], ['Home', 1, 14],
  ['Tab', 2, 0], ['Q', 2, 1], ['W', 2, 2], ['E', 2, 3], ['R', 2, 4],
  ['T', 2, 5], ['Y', 2, 6], ['U', 2, 7], ['I', 2, 8], ['O', 2, 9],
  ['P', 2, 10], ['[', 2, 11], [']', 2, 12], ['\\', 2, 13], ['PgUp', 2, 14],
  ['Caps', 3, 0], ['A', 3, 1], ['S', 3, 2], ['D', 3, 3], ['F', 3, 4],
  ['G', 3, 5], ['H', 3, 6], ['J', 3, 7], ['K', 3, 8], ['L', 3, 9],
  [';', 3, 10], ["'", 3, 11], ['Enter', 3, 13], ['PgDn', 3, 14],
  ['LShift', 4, 0], ['Z', 4, 1], ['X', 4, 2], ['C', 4, 3], ['V', 4, 4],
  ['B', 4, 5], ['N', 4, 6], ['M', 4, 7], [',', 4, 8], ['.', 4, 9],
  ['/', 4, 10], ['RShift', 4, 11], ['Up', 4, 13], ['End', 4, 14],
  ['LCtrl', 5, 0], ['Win', 5, 1], ['LAlt', 5, 2], ['Space', 5, 5],
  ['RAlt', 5, 9], ['Fn', 5, 10], ['RCtrl', 5, 11], ['Left', 5, 12],
  ['Down', 5, 13], ['Right', 5, 14],
];
const KEY_COLUMNS = 15;
const KEY_ROWS = 6;
// Only for drawing: where a few wide keys start and how many columns they
// cover. Their colour still comes from their own column.
const WIDE_KEYS = { Space: [3, 6], Enter: [12, 2], RShift: [11, 2] };

const currentEffect = () => state.config.effects[state.effectIndex];

function effectSwatch(effect) {
  const colors = effect.colors.length ? effect.colors : ['#000000'];
  const paint = colors.length === 1 ? esc(colors[0]) : `linear-gradient(135deg, ${colors.map(esc).join(', ')})`;
  return `<span class="swatch" style="background:${paint}"></span>`;
}

function freeEffectSlot(except) {
  const used = new Set(state.config.effects.filter((e) => e !== except).map((e) => e.slot));
  for (let slot = 1; slot <= EFFECT_SLOTS; slot++) if (!used.has(slot)) return slot;
  return null;
}

function uniqueEffectName(base) {
  const names = new Set(state.config.effects.map((e) => e.name.trim().toLowerCase()));
  if (!names.has(base.trim().toLowerCase())) return base;
  for (let n = 2; ; n++) {
    const name = `${base} ${n}`;
    if (!names.has(name.toLowerCase())) return name;
  }
}

function effectsPage() {
  const effects = state.config.effects;
  state.effectIndex = Math.min(state.effectIndex, Math.max(0, effects.length - 1));
  const list = effects.map((e, i) => `
    <button class="mode-item ${i === state.effectIndex ? 'active' : ''}" data-cmd="select-effect" data-index="${i}">
      <span class="mode-name">${effectSwatch(e)}${esc(e.name) || t('modes.unnamed')}</span>
      <span class="mode-meta">${t('effects.slotN', { slot: e.slot })} · ${PATTERNS.includes(e.pattern) ? t(`effects.patterns.${e.pattern}`) : esc(e.pattern)}</span>
    </button>`).join('');
  const full = effects.length >= EFFECT_SLOTS;
  const effect = currentEffect();
  if (!effectPreview.running) {
    effectPreview.running = true;
    requestAnimationFrame(drawEffectPreview);
  }
  return `<div class="modes">
    <div class="mode-list">
      ${list}
      <button class="btn ghost" data-cmd="add-effect" ${full ? 'disabled' : ''}>${icon('plus')} ${t('effects.newEffect')}</button>
      ${full ? `<small class="hint">${t('effects.slotsFull')}</small>` : ''}
    </div>
    ${effect ? effectEditor(effect, state.effectIndex, full) : `<div class="card muted">${t('effects.empty')}</div>`}
  </div>`;
}

const effectRange = (i, key, min, max, value) => `<div class="range-field">
  <div><strong>${t(`effects.${key}`)}</strong><small>${t(`effects.${key}Hint`)}</small></div>
  <input type="range" min="${min}" max="${max}" step="1" value="${esc(value)}" data-bind="effects.${i}.${key}" data-type="int" data-plain="1" aria-label="${esc(tPlain(`effects.${key}`))}">
  <output>${esc(value)}</output>
</div>`;

function effectEditor(effect, i, full) {
  const taken = new Map(state.config.effects.filter((e) => e !== effect).map((e) => [e.slot, e.name]));
  const slots = Array.from({ length: EFFECT_SLOTS }, (_, n) => n + 1).map((slot) => {
    const label = taken.has(slot) ? t('effects.slotTaken', { slot, name: taken.get(slot) }) : t('effects.slotN', { slot });
    return `<option value="${slot}" ${slot === effect.slot ? 'selected' : ''}>${label}</option>`;
  }).join('');
  const patterns = PATTERNS.map((p) => `<option value="${p}" ${p === effect.pattern ? 'selected' : ''}>${t(`effects.patterns.${p}`)}</option>`).join('');
  const colors = effect.colors.map((c, j) => `<span class="effect-color">
      <input type="color" data-bind="effects.${i}.colors.${j}" value="${esc(String(c).toLowerCase())}" aria-label="${esc(tPlain('effects.colorN', { n: j + 1 }))}">
      ${effect.colors.length > 1 ? `<button class="icon-btn" data-cmd="effect-color-remove" data-index="${j}" title="${esc(tPlain('effects.removeColor'))}" aria-label="${esc(tPlain('effects.removeColor'))}">×</button>` : ''}
    </span>`).join('');
  const patternHint = PATTERNS.includes(effect.pattern) ? t(`effects.patternHints.${effect.pattern}`) : '';
  return `<section class="card editor">
    <div class="editor-head">
      <label class="field grow"><span>${t('effects.name')}</span>
        <input type="text" data-bind="effects.${i}.name" value="${esc(effect.name)}" maxlength="40" spellcheck="false">
      </label>
      <div class="head-actions">
        <button class="btn" data-cmd="duplicate-effect" ${full ? 'disabled' : ''}>${t('buttons.duplicate')}</button>
        <button class="btn danger" data-cmd="delete-effect">${t('buttons.delete')}</button>
      </div>
    </div>
    <div class="effect-preview">
      <canvas id="effect-preview" aria-label="${esc(tPlain('effects.preview'))}"></canvas>
      <div class="effect-preview-foot">
        <small class="hint">${t('effects.previewHint')}</small>
        <button class="btn small" data-cmd="effect-restart">${t('effects.restart')}</button>
      </div>
    </div>
    <div class="setting-row">
      <div><strong>${t('effects.slot')}</strong><small>${t('effects.slotHint')}</small></div>
      <select data-cmd="effect-slot" aria-label="${esc(tPlain('effects.slot'))}">${slots}</select>
    </div>
    <div class="setting-row">
      <div><strong>${t('effects.pattern')}</strong><small>${patternHint}</small></div>
      <select data-cmd="effect-pattern" aria-label="${esc(tPlain('effects.pattern'))}">${patterns}</select>
    </div>
    <div class="setting-row">
      <div><strong>${t('effects.colors')}</strong><small>${t('effects.colorsHint')}</small></div>
      <div class="color-controls">
        ${colors}
        ${effect.colors.length < EFFECT_MAX_COLORS ? `<button class="btn small" data-cmd="effect-color-add">${icon('plus')} ${t('effects.addColor')}</button>` : ''}
      </div>
    </div>
    ${effectRange(i, 'speed', 1, 10, effect.speed)}
    ${effectRange(i, 'brightness', 1, 100, effect.brightness)}
    <div class="setting-row">
      <div><strong>${t('effects.duration')}</strong><small>${t('effects.durationHint')}</small></div>
      ${numberParam(`effects.${i}.duration_ms`, effect.duration_ms, 0, 60000, t('effects.ms'))}
    </div>
    <div class="setting-row effect-when">
      <div><strong>${t('effects.when')}</strong><small>${t('effects.whenHint')}</small></div>
      <div class="when-list">
        ${(effect.when ?? []).map((w, j) => `
          <span class="chip">${esc(situationLabel(w))}
            <button class="icon-btn" data-cmd="effect-when-remove" data-index="${j}" title="${esc(tPlain('effects.whenRemove'))}" aria-label="${esc(tPlain('effects.whenRemove'))}">×</button>
          </span>`).join('') || `<small class="hint">${t('effects.whenNone')}</small>`}
        <div class="when-add">
          <button class="btn small" data-cmd="effect-when-app">${icon('plus')} ${t('effects.whenApp')}</button>
          <select data-cmd="effect-when-mode" aria-label="${esc(tPlain('effects.whenMode'))}">
            <option value="">${t('effects.whenMode')}</option>
            ${state.config.modes.map((m) => `<option value="${esc(m.name)}">${esc(m.name)}</option>`).join('')}
          </select>
          <select data-cmd="effect-when-game" aria-label="${esc(tPlain('effects.whenGame'))}">
            <option value="">${t('effects.whenGame')}</option>
            ${Object.entries(GAME_SITUATIONS).map(([group, tests]) => `
              <optgroup label="${esc(tPlain(`effects.game.${group}`))}">
                ${tests.map((test) => `<option value="${esc(test)}">${esc(gameLabel(test))}</option>`).join('')}
              </optgroup>`).join('')}
          </select>
        </div>
        ${(effect.when ?? []).some((w) => String(w).startsWith('game:')) ? `<small class="hint">${t('effects.whenGameHint')}</small>` : ''}
      </div>
    </div>
    <div class="apply-row">
      <button class="btn primary" data-cmd="effect-play" ${state.effectBusy ? 'disabled' : ''}>${t('effects.play')}</button>
      <button class="btn" data-cmd="effect-stop" ${state.effectBusy ? 'disabled' : ''}>${t('effects.stop')}</button>
      <small class="hint">${firmwareLine()}</small>
    </div>
  </section>`;
}

/** The `game:` situations offered in the picker; see knob-core game.rs. */
const GAME_SITUATIONS = {
  state: ['valheim', 'effects=burning', 'effects=frost', 'effects=freezing', 'effects=poison',
    'effects=lightning', 'effects=resting', 'effects=wet', 'effects=cold', 'effects=smoked',
    'health<0.25', 'dead'],
  surroundings: ['boss', 'night', 'weather~thunder', 'biome=meadows', 'biome=blackforest',
    'biome=swamp', 'biome=mountain', 'biome=plains', 'biome=mistlands', 'biome=ashlands',
    'biome=deepnorth', 'biome=ocean'],
  moments: ['event=hurt', 'event=hit', 'event=kill', 'event=bosskill', 'event=parry',
    'event=levelup', 'event=power_used', 'event=power_ready'],
};

/** "effects=burning" → "Burning"; anything not in the picker is shown as written. */
function gameLabel(test) {
  const known = Object.values(GAME_SITUATIONS).some((tests) => tests.includes(test));
  return known ? tPlain(`effects.game.${test.replace(/[^a-z0-9]/g, '_')}`) : test;
}

/** "app:valheim.exe" → "App: valheim.exe"; "mode:Volume" → "Mode: Volume"; "game:boss" → "Game: Boss fight". */
function situationLabel(when) {
  const [kind, ...rest] = String(when).split(':');
  const value = rest.join(':').trim();
  if (kind === 'app') return tPlain('effects.whenAppLabel', { app: value });
  if (kind === 'mode') return tPlain('effects.whenModeLabel', { mode: value });
  if (kind === 'game') return tPlain('effects.whenGameLabel', { what: gameLabel(value) });
  return when;
}

/** One line about the keyboard's firmware; asks it once per visit. */
function firmwareLine() {
  const fw = state.effectFw;
  if (fw === undefined) {
    state.effectFw = null;
    invoke('effect_firmware')
      .then((r) => { state.effectFw = r; })
      .catch(() => { state.effectFw = { kind: 'unknown', error: 'other' }; })
      .finally(() => { if (state.page === 'effects') render(); });
  }
  if (!fw) return t('effects.fw.checking');
  if (fw.kind === 'live' && fw.effects) return t('effects.fw.ready', { version: fw.version });
  if (fw.kind === 'live') return t('effects.fw.old', { version: fw.version });
  if (fw.kind === 'stock') return t('effects.fw.stock');
  return t('effects.fw.unknown');
}

async function effectCommand(command, args, done) {
  if (state.effectBusy) return;
  state.effectBusy = true;
  render();
  try {
    await invoke(command, args);
    if (done) toast(done);
  } catch (e) {
    const known = ['no_keyboard', 'no_reply', 'no_engine'].includes(e);
    toast(known ? tPlain(`effects.errors.${e}`) : tPlain('effects.errors.other', { error: e }), true);
    state.effectFw = undefined; // ask again: it may have been asleep or reflashed
  }
  state.effectBusy = false;
  render();
}

// ---- preview: the formulas from docs/effects-format.md

// One animation loop at a time; it stops by itself when the page is left.
const effectPreview = { running: false, index: -1, start: 0 };

function hexRgb(hex) {
  const m = /^#([0-9a-f]{6})$/i.exec(String(hex));
  if (!m) return [0, 0, 0];
  const n = parseInt(m[1], 16);
  return [n >> 16, (n >> 8) & 255, n & 255];
}

const mix = (a, b, f) => [0, 1, 2].map((k) => a[k] + (b[k] - a[k]) * f);
const frac = (x) => x - Math.floor(x);

/** Blends the colours evenly over u in 0..1; `loop` joins the last back to the first. */
function palette(colors, u, loop = false) {
  if (colors.length === 1) return colors[0];
  const segments = loop ? colors.length : colors.length - 1;
  const pos = Math.min(Math.max(u, 0), loop ? 0.999999 : 1) * segments;
  const i = Math.min(Math.floor(pos), segments - 1);
  return mix(colors[i], colors[(i + 1) % colors.length], pos - i);
}

/** Repeatable 0..1 noise for a key and a step number. */
function noise(key, step, salt = 0) {
  let h = (key * 374761393 + step * 668265263 + salt * 2246822519) >>> 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177) >>> 0;
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/** The colour of one key `t` ms after the effect started. */
function effectKeyColor(effect, colors, key, x, t) {
  const P = 4000 / Math.min(Math.max(effect.speed || 1, 1), 10);
  switch (effect.pattern) {
    case 'pulse': {
      const f = (1 - Math.cos((2 * Math.PI * t) / P)) / 2;
      return mix(colors[1] ?? [0, 0, 0], colors[0], f);
    }
    case 'wave':
      return palette(colors, frac(x - t / P), true);
    case 'flicker': {
      const roll = P / 8;
      const step = Math.floor(t / roll);
      const s = (t - step * roll) / roll;
      const smooth = s * s * (3 - 2 * s);
      const v = noise(key, step) + (noise(key, step + 1) - noise(key, step)) * smooth;
      return palette(colors, v).map((c) => c * (0.55 + 0.45 * v));
    }
    case 'sparkle': {
      const q = P / 4;
      const chance = (effect.speed * 2) / 100;
      const flashes = colors.length > 1 ? colors.slice(1) : [[255, 255, 255]];
      let color = colors[0];
      const now = Math.floor(t / q);
      for (const n of [now - 1, now]) {
        if (n < 0 || noise(key, n, 1) >= chance) continue;
        const age = t - (n + noise(key, n, 2)) * q;
        if (age < 0 || age >= q) continue;
        const flash = flashes[Math.floor(noise(key, n, 3) * flashes.length)];
        color = mix(color, flash, 1 - age / q);
      }
      return color;
    }
    default:
      return colors[0];
  }
}

function drawEffectPreview(now) {
  const canvas = document.getElementById('effect-preview');
  const effect = state.page === 'effects' ? currentEffect() : null;
  if (!canvas || !effect) {
    effectPreview.running = false; // effectsPage() starts it again
    return;
  }
  if (effectPreview.index !== state.effectIndex) {
    effectPreview.index = state.effectIndex;
    effectPreview.start = now;
  }
  const ratio = window.devicePixelRatio || 1;
  const width = Math.round(canvas.clientWidth * ratio);
  const height = Math.round(canvas.clientHeight * ratio);
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  const ctx = canvas.getContext('2d');
  ctx.fillStyle = '#121418';
  ctx.fillRect(0, 0, width, height);

  // With a duration the effect ends by itself; the preview shows the keys
  // dark for a moment, then plays it again.
  let t = now - effectPreview.start;
  const duration = effect.duration_ms || 0;
  if (duration > 0 && t > duration + 1200) {
    effectPreview.start = now;
    t = 0;
  }
  const over = duration > 0 && t > duration;
  const colors = effect.colors.length ? effect.colors.map(hexRgb) : [[0, 0, 0]];
  const scale = Math.min(Math.max(effect.brightness || 0, 0), 100) / 100;
  const pad = 10 * ratio;
  const unit = Math.min((width - 2 * pad) / KEY_COLUMNS, (height - 2 * pad) / KEY_ROWS);
  const left = (width - unit * KEY_COLUMNS) / 2;
  const top = (height - unit * KEY_ROWS) / 2;
  const gap = Math.max(2, unit * 0.1);
  KEYBOARD_KEYS.forEach(([name, row, col], key) => {
    const [startCol, span] = WIDE_KEYS[name] ?? [col, 1];
    const rgb = over ? [38, 41, 48] : effectKeyColor(effect, colors, key, col / (KEY_COLUMNS - 1), t).map((c) => c * scale);
    ctx.fillStyle = `rgb(${rgb.map((c) => Math.round(Math.min(Math.max(c, 0), 255))).join(',')})`;
    const x = left + startCol * unit + gap / 2;
    const y = top + row * unit + gap / 2;
    const w = span * unit - gap;
    const h = unit - gap;
    ctx.beginPath();
    if (ctx.roundRect) ctx.roundRect(x, y, w, h, unit * 0.12);
    else ctx.rect(x, y, w, h);
    ctx.fill();
  });
  requestAnimationFrame(drawEffectPreview);
}

// ---------- snippets page (text expansion)
//
// Kept apart from state.config: snippets live in their own file, encrypted
// for this Windows user, and every create, edit or delete is saved at once.
// The decrypted list is only held while this page is open.

const snip = {
  store: null, // { enabled, exclude, snippets } once loaded
  error: null,
  loading: false,
  index: null, // the snippet being edited; null with a draft = a new one
  draft: null,
  reveal: false, // show a secret snippet's text
  problems: [],
  busy: false,
  filter: '',
};
const SNIP_PLACEHOLDERS = ['{date}', '{time}', '{cursor}'];

const blankSnippet = () => ({ trigger: ';', text: '', enabled: true, secret: false, apps: [] });

async function loadSnippets() {
  snip.loading = true;
  try {
    snip.store = await invoke('snippets_load');
    snip.error = null;
    if (snip.draft == null && snip.store.snippets.length) selectSnippet(0);
  } catch (e) {
    snip.store = null;
    snip.error = String(e);
  }
  snip.loading = false;
  if (state.page === 'snippets') render();
}

/** Forgets the decrypted list (on leaving the page). */
function forgetSnippets() {
  Object.assign(snip, { store: null, error: null, index: null, draft: null, reveal: false, problems: [], filter: '' });
}

function selectSnippet(i) {
  snip.index = i;
  snip.draft = i == null ? blankSnippet() : clone(snip.store.snippets[i]);
  snip.reveal = false;
  snip.problems = [];
}

function snippetDirty() {
  if (!snip.draft) return false;
  const saved = snip.index == null ? blankSnippet() : snip.store?.snippets[snip.index];
  return JSON.stringify(saved) !== JSON.stringify(snip.draft);
}

/** True when it's fine to drop the draft (nothing changed, or the user agrees). */
function leaveSnippetOk() {
  return !snippetDirty() || confirm(tPlain('snippets.confirmLeave'));
}

function snippetPreview(text) {
  const line = String(text).split('\n')[0];
  return line.length > 38 ? `${line.slice(0, 37)}…` : line;
}

function snippetsPage() {
  if (snip.error) {
    return `<div class="banner error">${t('snippets.loadError', { error: snip.error })}</div>
      <p class="hint">${t('snippets.loadErrorHint')}</p>`;
  }
  if (!snip.store) {
    if (!snip.loading) loadSnippets();
    return `<div class="card muted">${t('snippets.loading')}</div>`;
  }
  const store = snip.store;
  const f = snip.filter.trim().toLowerCase();
  const list = store.snippets
    .map((s, i) => ({ s, i }))
    .filter(({ s }) => !f || s.trigger.toLowerCase().includes(f) || (!s.secret && s.text.toLowerCase().includes(f)))
    .map(({ s, i }) => `
    <button class="mode-item ${i === snip.index ? 'active' : ''} ${s.enabled ? '' : 'off'}" data-cmd="snip-select" data-index="${i}">
      <span class="mode-name snip-trigger">${esc(s.trigger)}</span>
      <span class="mode-meta">${s.secret ? t('snippets.hiddenText') : esc(snippetPreview(s.text))}${s.enabled ? '' : ` · ${t('snippets.off')}`}</span>
    </button>`).join('');
  const remove = esc(tPlain('apps.remove'));
  const exclude = store.exclude.map((exe, k) => `<span class="chip">${esc(exe)}<button data-cmd="snip-exclude-remove" data-app="${k}" title="${remove}">×</button></span>`).join('');
  return `<section class="card">
      <div class="setting-row">
        <div><strong>${t('snippets.master')}</strong><small>${t('snippets.masterHint')}</small></div>
        <label class="param check"><input type="checkbox" data-cmd="snip-master" ${store.enabled ? 'checked' : ''} ${snip.busy ? 'disabled' : ''}><span>${t('snippets.on')}</span></label>
      </div>
      <div class="setting-row">
        <div><strong>${t('snippets.exclude')}</strong><small>${t('snippets.excludeHint')}</small></div>
        <div class="chips">${exclude}<button class="btn small" data-cmd="snip-exclude-add">${icon('plus')} ${t('apps.addApp')}</button></div>
      </div>
      <p class="hint">${t('snippets.security')}</p>
    </section>
    <div class="modes">
      <div class="mode-list">
        ${store.snippets.length > 6 ? `<input type="search" class="snip-search" data-snip-filter placeholder="${esc(tPlain('snippets.search'))}" value="${esc(snip.filter)}">` : ''}
        ${list || (store.snippets.length ? `<p class="hint pad">${t('snippets.noMatch')}</p>` : '')}
        <button class="btn ghost" data-cmd="snip-new">${icon('plus')} ${t('snippets.new')}</button>
      </div>
      ${snip.draft ? snippetEditor() : `<div class="card muted">${t('snippets.empty')}</div>`}
    </div>`;
}

function snippetEditor() {
  const d = snip.draft;
  const isNew = snip.index == null;
  const hidden = d.secret && !snip.reveal;
  const dirty = snippetDirty();
  const remove = esc(tPlain('apps.remove'));
  const apps = d.apps.length
    ? d.apps.map((exe, k) => `<span class="chip">${esc(exe)}<button data-cmd="snip-remove-app" data-app="${k}" title="${remove}">×</button></span>`).join('')
    : `<span class="chip all">${t('apps.everyApp')}</span>`;
  // A hidden text is not even put in the page until "Show" is clicked.
  const textField = hidden
    ? `<div class="snip-hidden">${t('snippets.hiddenNote')}</div>`
    : `<textarea class="snip-text" data-snip="text" rows="6" spellcheck="false" autocomplete="off" maxlength="4000">${esc(d.text)}</textarea>`;
  return `<section class="card editor">
    <div class="editor-head">
      <label class="field grow"><span>${t('snippets.trigger')}</span>
        <input type="text" class="snip-trigger" data-snip="trigger" value="${esc(d.trigger)}" maxlength="32" spellcheck="false" autocomplete="off">
      </label>
      ${isNew ? '' : `<div class="head-actions"><button class="btn danger" data-cmd="snip-delete" ${snip.busy ? 'disabled' : ''}>${t('buttons.delete')}</button></div>`}
    </div>
    <p class="hint">${t('snippets.triggerHint')}</p>
    <div class="field"><span>${t('snippets.text')}</span>${textField}</div>
    <div class="snip-tools">
      ${hidden ? '' : SNIP_PLACEHOLDERS.map((p) => `<button class="btn small" data-cmd="snip-insert" data-text="${p}">${p}</button>`).join('')}
      ${d.secret ? `<button class="btn small" data-cmd="snip-reveal">${snip.reveal ? t('snippets.hide') : t('snippets.show')}</button>` : ''}
    </div>
    ${hidden ? '' : `<p class="hint">${t('snippets.placeholders')}</p>`}
    <div class="setting-row">
      <div><strong>${t('snippets.where')}</strong><small>${t('snippets.whereHint')}</small></div>
      <div class="chips">${apps}<button class="btn small" data-cmd="snip-add-app">${icon('plus')} ${d.apps.length ? t('apps.addApp') : t('remap.onlyIn')}</button></div>
    </div>
    <div class="setting-row">
      <div><strong>${t('snippets.options')}</strong></div>
      <div class="color-controls">
        <label class="param check"><input type="checkbox" data-cmd="snip-flag" data-flag="enabled" ${d.enabled ? 'checked' : ''}><span>${t('snippets.enabled')}</span></label>
        <label class="param check" title="${esc(tPlain('snippets.secretHint'))}"><input type="checkbox" data-cmd="snip-flag" data-flag="secret" ${d.secret ? 'checked' : ''}><span>${t('snippets.secret')}</span></label>
      </div>
    </div>
    <div id="snip-problems">${snippetProblems()}</div>
    <div class="apply-row">
      <button class="btn primary" data-cmd="snip-save" ${snip.busy || !dirty ? 'disabled' : ''}>${isNew ? t('snippets.create') : t('buttons.save')}</button>
      ${isNew
        ? `<button class="btn" data-cmd="snip-cancel">${t('snippets.cancel')}</button>`
        : `<button class="btn" data-cmd="snip-revert" ${dirty ? '' : 'disabled'}>${t('buttons.discard')}</button>`}
      <small class="hint">${t('snippets.notHere')}</small>
    </div>
  </section>`;
}

function snippetProblems() {
  return snip.problems.length
    ? `<div class="banner error">${t('banners.problems')}<ul>${snip.problems.map((p) => `<li>${esc(p)}</li>`).join('')}</ul></div>`
    : '';
}

/** The list as it would be with the draft saved, and the draft's index in it. */
function storeWithDraft() {
  const next = clone(snip.store);
  const draft = { ...clone(snip.draft), trigger: snip.draft.trigger.trim() };
  draft.apps = draft.apps.map((a) => a.trim().toLowerCase()).filter(Boolean);
  let index = snip.index;
  if (index == null) {
    next.snippets.push(draft);
    index = next.snippets.length - 1;
  } else {
    next.snippets[index] = draft;
  }
  return { next, index };
}

let snipCheckTimer = null;
/** After typing in the editor: keep the buttons and problems current without redrawing. */
function snippetEdited() {
  const dirty = snippetDirty();
  const saveBtn = $('[data-cmd="snip-save"]');
  if (saveBtn) saveBtn.disabled = snip.busy || !dirty;
  const revert = $('[data-cmd="snip-revert"]');
  if (revert) revert.disabled = !dirty;
  clearTimeout(snipCheckTimer);
  snipCheckTimer = setTimeout(async () => {
    if (!snip.draft) return;
    try {
      snip.problems = await invoke('snippets_check', { store: storeWithDraft().next });
    } catch (e) {
      snip.problems = [String(e)];
    }
    const box = $('#snip-problems');
    if (box) box.innerHTML = snippetProblems();
  }, 200);
}

/** Checks, encrypts and saves `next`. Returns true when saved. */
async function saveSnippets(next, message) {
  snip.busy = true;
  render();
  let ok = false;
  try {
    const problems = await invoke('snippets_check', { store: next });
    if (problems.length) {
      snip.problems = problems;
    } else {
      await invoke('snippets_save', { store: next });
      snip.store = next;
      snip.problems = [];
      ok = true;
      toast(tPlain(message));
    }
  } catch (e) {
    toast(tPlain('snippets.saveFailed', { error: e }), true);
  }
  snip.busy = false;
  render();
  return ok;
}

const snippetCommands = {
  'snip-select': (el) => {
    const i = Number(el.dataset.index);
    if (i === snip.index || !leaveSnippetOk()) return;
    selectSnippet(i);
    render();
  },
  'snip-new': () => {
    if (!leaveSnippetOk()) return;
    selectSnippet(null);
    render();
    const input = $('input[data-snip="trigger"]');
    input?.focus();
    input?.setSelectionRange(input.value.length, input.value.length);
  },
  'snip-cancel': () => {
    if (!leaveSnippetOk()) return;
    if (snip.store.snippets.length) selectSnippet(0);
    else snip.draft = null;
    render();
  },
  'snip-revert': () => { selectSnippet(snip.index); render(); },
  'snip-save': async () => {
    const { next, index } = storeWithDraft();
    if (await saveSnippets(next, snip.index == null ? 'snippets.created' : 'snippets.saved')) {
      const reveal = snip.reveal;
      selectSnippet(index);
      snip.reveal = reveal;
      render();
    }
  },
  'snip-delete': async () => {
    const trigger = snip.store.snippets[snip.index]?.trigger;
    if (!confirm(tPlain('snippets.confirmDelete', { trigger }))) return;
    const next = clone(snip.store);
    next.snippets.splice(snip.index, 1);
    const at = snip.index;
    if (await saveSnippets(next, 'snippets.deleted')) {
      if (next.snippets.length) selectSnippet(Math.min(at, next.snippets.length - 1));
      else snip.draft = null;
      render();
    }
  },
  'snip-reveal': () => { snip.reveal = !snip.reveal; render(); },
  'snip-insert': (el) => {
    const area = $('textarea[data-snip="text"]');
    if (!area) return;
    area.focus();
    area.setRangeText(el.dataset.text, area.selectionStart, area.selectionEnd, 'end');
    snip.draft.text = area.value;
    snippetEdited();
  },
  'snip-add-app': () => openPicker(0, 'snippet'),
  'snip-remove-app': (el) => {
    snip.draft.apps.splice(Number(el.dataset.app), 1);
    render();
    snippetEdited();
  },
  'snip-exclude-add': () => openPicker(0, 'snippetExclude'),
  'snip-exclude-remove': (el) => {
    const next = clone(snip.store);
    next.exclude.splice(Number(el.dataset.app), 1);
    saveSnippets(next, 'snippets.savedSettings');
  },
};

const snippetChangeCommands = {
  'snip-master': (el) => {
    const next = clone(snip.store);
    next.enabled = el.checked;
    saveSnippets(next, 'snippets.savedSettings');
  },
  'snip-flag': (el) => {
    snip.draft[el.dataset.flag] = el.checked;
    if (el.dataset.flag === 'secret') snip.reveal = el.checked;
    render();
    snippetEdited();
  },
};

// ---------- macros page

const currentMacro = () => state.config.macros[state.macroIndex];
const STEP_TYPES = ['keys', 'text', 'wait', 'click', 'key_down', 'key_up', 'move', 'scroll'];

function macrosPage() {
  const macros = state.config.macros;
  state.macroIndex = Math.min(state.macroIndex, Math.max(0, macros.length - 1));
  const list = macros.map((m, i) => `
    <button class="mode-item ${i === state.macroIndex ? 'active' : ''}" data-cmd="select-macro" data-index="${i}">
      <span class="mode-name">${esc(m.name) || t('modes.unnamed')}</span>
      <span class="mode-meta">${m.hotkey ? esc(m.hotkey) : t('macros.noHotkey')}</span>
    </button>`).join('');
  const item = currentMacro();
  return `<div class="modes">
    <div class="mode-list">
      ${list}
      <button class="btn ghost" data-cmd="add-macro">${icon('plus')} ${t('macros.newMacro')}</button>
    </div>
    ${item ? macroEditor(item, state.macroIndex) : `<div class="card muted">${t('macros.empty')}</div>`}
  </div>`;
}

function stepFields(step, path) {
  if (step.step === 'keys') return keyParam(`${path}.keys`, step.keys, t('macros.steps.keys'));
  if (step.step === 'key_down' || step.step === 'key_up') {
    return `<div class="param keyfield single">
      <span class="param-label">${t(`macros.steps.${step.step}`)}</span>
      <input class="keycap" readonly data-keycap="${path}.key" data-keymode="single" value="${esc(step.key)}" title="${esc(tPlain('remap.keyTitle'))}" aria-label="${esc(tPlain(`macros.steps.${step.step}`))}">
    </div>`;
  }
  if (step.step === 'move') {
    return `<span class="step-kind">${t('macros.steps.move')}</span>
      ${numberParam(`${path}.x`, step.x, -100000, 100000, 'x')}
      ${numberParam(`${path}.y`, step.y, -100000, 100000, 'y')}
      ${checkParam(`${path}.relative`, step.relative, t('macros.relative'))}`;
  }
  if (step.step === 'scroll') {
    return `<span class="step-kind">${t('macros.steps.scroll')}</span>
      ${numberParam(`${path}.notches`, step.notches, -100, 100, t('macros.notches'))}
      ${checkParam(`${path}.horizontal`, step.horizontal, t('params.sideways'))}`;
  }
  if (step.step === 'text') {
    return `<label class="field grow"><span>${t('macros.steps.text')}</span>
      <input type="text" data-bind="${path}.text" value="${esc(step.text)}" spellcheck="false"></label>`;
  }
  if (step.step === 'wait') return numberParam(`${path}.ms`, step.ms, 0, 60000, t('macros.ms'));
  return `<label class="param"><select data-bind="${path}.button" aria-label="${esc(tPlain('macros.steps.click'))}">
    ${['left', 'middle', 'right'].map((b) => `<option value="${b}" ${step.button === b ? 'selected' : ''}>${t(`params.mouse.${b}`)}</option>`).join('')}
  </select></label>`;
}

function macroEditor(item, i) {
  const recording = state.recording?.macro === i;
  const count = item.steps.length;
  const steps = item.steps.map((step, k) => {
    const path = `macros.${i}.steps.${k}`;
    return `<div class="step-row">
      <span class="order">${k + 1}</span>
      <div class="step-body">${stepFields(step, path)}</div>
      <div class="head-actions">
        <button class="icon-btn" data-cmd="step-move" data-index="${k}" data-delta="-1" title="${esc(tPlain('apps.modeEarlier'))}" ${k === 0 ? 'disabled' : ''}>↑</button>
        <button class="icon-btn" data-cmd="step-move" data-index="${k}" data-delta="1" title="${esc(tPlain('apps.modeLater'))}" ${k === count - 1 ? 'disabled' : ''}>↓</button>
        <button class="icon-btn danger" data-cmd="step-remove" data-index="${k}" title="${esc(tPlain('apps.remove'))}">✕</button>
      </div>
    </div>`;
  }).join('');
  const addOptions = STEP_TYPES.map((s) => `<option value="${s}">${t(`macros.steps.${s}`)}</option>`).join('');
  return `<section class="card editor">
    <div class="editor-head">
      <label class="field grow"><span>${t('macros.name')}</span>
        <input type="text" data-bind="macros.${i}.name" value="${esc(item.name)}" maxlength="40" spellcheck="false">
      </label>
      <div class="head-actions">
        <button class="btn" data-cmd="duplicate-macro">${t('buttons.duplicate')}</button>
        <button class="btn danger" data-cmd="delete-macro">${t('buttons.delete')}</button>
      </div>
    </div>
    <div class="setting-row">
      <div><strong>${t('macros.hotkey')}</strong><small>${t('macros.hotkeyHint')}</small></div>
      <div class="color-controls">
        <input class="keycap" readonly data-keycap="macros.${i}.hotkey" value="${esc(item.hotkey ?? '')}" placeholder="${esc(tPlain('macros.none'))}" title="${esc(tPlain('params.keycapTitle'))}">
        ${item.hotkey ? `<button class="icon-btn" data-cmd="hotkey-clear" title="${esc(tPlain('apps.remove'))}">✕</button>` : ''}
      </div>
    </div>
    <div class="setting-row">
      <div><strong>${t('macros.where')}</strong><small>${t('macros.whereHint')}</small></div>
      <div class="chips">${macroApps(item, i)}</div>
    </div>
    <div class="setting-row">
      <div><strong>${t('macros.repeat')}</strong></div>
      <div class="color-controls">
        ${checkParam(`macros.${i}.until_stopped`, item.until_stopped, t('macros.untilStopped'))}
        ${item.until_stopped ? '' : numberParam(`macros.${i}.repeat`, item.repeat, 1, 100, t('macros.times'))}
      </div>
    </div>
    <div class="steps-head">
      <h2>${t('macros.stepsTitle')}</h2>
      <div class="head-actions">
        <button class="btn ${recording ? 'danger' : ''}" data-cmd="macro-record">${recording ? t('macros.stop') : t('macros.record')}</button>
        <select data-cmd="step-add" aria-label="${esc(tPlain('macros.addStep'))}">
          <option value="">${t('macros.addStep')}</option>${addOptions}
        </select>
      </div>
    </div>
    ${recording ? `<div class="banner warn">${t('macros.recording')}</div>` : ''}
    ${steps || `<p class="hint">${t('macros.noSteps')}</p>`}
    <p class="hint">${t('macros.note')}</p>
  </section>`;
}

function macroApps(item, i) {
  const remove = esc(tPlain('apps.remove'));
  const chips = item.apps.length
    ? item.apps.map((exe, k) => `<span class="chip">${esc(exe)}<button data-cmd="macro-remove-app" data-index="${i}" data-app="${k}" title="${remove}">×</button></span>`).join('')
    : `<span class="chip all">${t('apps.everyApp')}</span>`;
  return `${chips}<button class="btn small" data-cmd="open-macro-picker" data-index="${i}">${icon('plus')} ${item.apps.length ? t('apps.addApp') : t('remap.onlyIn')}</button>`;
}

function renameMacroRefs(from, to) {
  for (const mode of state.config.modes) {
    for (const bindings of [mode, mode.ctrl, mode.shift, mode.alt, mode.win]) {
      if (!bindings) continue;
      for (const g of GESTURES) {
        const action = bindings[g.key];
        if (action?.action === 'run_macro' && action.name === from) action.name = to;
      }
    }
  }
}

function uniqueMacroName(base) {
  const names = new Set(state.config.macros.map((m) => m.name));
  if (!names.has(base)) return base;
  for (let n = 2; ; n++) {
    const name = `${base} ${n}`;
    if (!names.has(name)) return name;
  }
}

function recordKey(e) {
  e.preventDefault();
  if (e.repeat) return;
  const combo = comboFromEvent(e);
  if (!combo) return;
  const steps = state.config.macros[state.recording.macro]?.steps;
  if (!steps || steps.length >= 498) return;
  const now = performance.now();
  if (state.recording.last) {
    const gap = Math.round((now - state.recording.last) / 10) * 10;
    if (gap >= 60) steps.push({ step: 'wait', ms: Math.min(gap, 60000) });
  }
  state.recording.last = now;
  steps.push({ step: 'keys', keys: combo });
  changed();
}

// ---------- mouse and hints page

function controlsPage() {
  const { mouse, hints } = state.config;
  const keycap = (path, value) => `<input class="keycap" readonly data-keycap="${path}" value="${esc(value ?? '')}" title="${esc(tPlain('params.keycapTitle'))}">`;
  const legend = ['arrows', 'rctrl', 'rshift', 'end', 'pages', 'home', 'esc']
    .map((key) => `<tr><td>${t(`controls.keys.${key}.key`)}</td><td>${t(`controls.keys.${key}.does`)}</td></tr>`).join('');
  return `<section class="card">
      <h2>${t('controls.mouseTitle')}</h2>
      <p class="hint">${t('controls.mouseLead')}</p>
      <label class="setting-row">
        <div><strong>${t('controls.enable')}</strong></div>
        <input type="checkbox" data-bind="mouse.enabled" data-type="bool" ${mouse.enabled ? 'checked' : ''}>
      </label>
      <div class="setting-row">
        <div><strong>${t('controls.toggle')}</strong><small>${t('controls.toggleHint')}</small></div>
        ${keycap('mouse.toggle', mouse.toggle)}
      </div>
      <div class="range-field">
        <div><strong>${t('controls.speed')}</strong><small>${t('controls.speedHint')}</small></div>
        <input type="range" min="200" max="5000" step="100" value="${esc(mouse.speed)}" data-bind="mouse.speed" data-type="int" data-plain="1" aria-label="${esc(tPlain('controls.speed'))}">
        <output>${esc(mouse.speed)}</output>
      </div>
      <table class="legend"><tbody>${legend}</tbody></table>
    </section>
    <section class="card">
      <h2>${t('controls.hintsTitle')}</h2>
      <p class="hint">${t('controls.hintsLead')}</p>
      <label class="setting-row">
        <div><strong>${t('controls.enable')}</strong></div>
        <input type="checkbox" data-bind="hints.enabled" data-type="bool" ${hints.enabled ? 'checked' : ''}>
      </label>
      <div class="setting-row">
        <div><strong>${t('controls.hintsKey')}</strong><small>${t('controls.toggleHint')}</small></div>
        ${keycap('hints.hotkey', hints.hotkey)}
      </div>
      ${hintAppearance(hints.style)}
    </section>`;
}

function hexAlpha(color, percent) {
  return color + Math.round(percent * 2.55).toString(16).padStart(2, '0');
}

/** The labels as they will look, on a sample background. */
function hintPreview(style) {
  const shadow = style.shadow ? 'box-shadow:0 1px 6px rgba(0,0,0,.4);' : '';
  const pill = `background:${esc(hexAlpha(style.background, style.opacity))};color:${esc(style.text)};`
    + `font-size:${Number(style.size)}px;border-radius:${Number(style.radius)}px;${shadow}`;
  return `<div class="hint-preview" id="hint-preview" aria-hidden="true">
    <span class="hint-sample" style="${pill}">A</span><span class="hint-word">Home</span>
    <span class="hint-sample" style="${pill}">S</span><span class="hint-word">Pricing</span>
    <span class="hint-sample" style="${pill}"><span style="color:${esc(style.typed)}">D</span>K</span><span class="hint-word">Sign in</span>
  </div>`;
}

function refreshHintPreview() {
  const preview = $('#hint-preview');
  if (preview) preview.outerHTML = hintPreview(state.config.hints.style);
}

function hintAppearance(style) {
  const color = (key) => `<label class="color-field"><span>${t(`controls.style.${key}`)}</span>
    <input type="color" data-bind="hints.style.${key}" value="${esc(style[key].toLowerCase())}" aria-label="${esc(tPlain(`controls.style.${key}`))}"></label>`;
  const range = (key, min, max) => `<div class="range-field">
    <div><strong>${t(`controls.style.${key}`)}</strong></div>
    <input type="range" min="${min}" max="${max}" step="1" value="${esc(style[key])}" data-bind="hints.style.${key}" data-type="int" data-plain="1" aria-label="${esc(tPlain(`controls.style.${key}`))}">
    <output>${esc(style[key])}</output>
  </div>`;
  const presets = Object.entries(HINT_PRESETS).map(([id, p]) => {
    const active = ['background', 'text', 'typed'].every((k) => style[k].toLowerCase() === p[k].toLowerCase());
    return `<button class="preset ${active ? 'active' : ''}" data-cmd="hint-preset" data-preset="${id}">
      <span class="hint-sample" style="background:${p.background};color:${p.text};border-radius:${p.radius}px">A</span>${t(`controls.style.presets.${id}`)}</button>`;
  }).join('');
  return `<h3 class="subhead">${t('controls.style.title')}</h3>
    ${hintPreview(style)}
    <div class="preset-row">${presets}</div>
    <div class="color-row">${color('background')}${color('text')}${color('typed')}</div>
    ${range('size', 10, 28)}
    ${range('radius', 0, 14)}
    ${range('opacity', 40, 100)}
    <div class="setting-row">
      <div><strong>${t('controls.style.position')}</strong></div>
      <select data-bind="hints.style.position" aria-label="${esc(tPlain('controls.style.position'))}">
        ${['left', 'center'].map((p) => `<option value="${p}" ${style.position === p ? 'selected' : ''}>${t(`controls.style.positions.${p}`)}</option>`).join('')}
      </select>
    </div>
    <label class="setting-row">
      <div><strong>${t('controls.style.shadow')}</strong></div>
      <input type="checkbox" data-bind="hints.style.shadow" data-type="bool" ${style.shadow ? 'checked' : ''}>
    </label>
    <div class="button-row"><button class="btn" data-cmd="hint-reset">${t('controls.style.reset')}</button></div>`;
}

// ---------- general page

function generalPage() {
  const { timing, ui } = state.config;
  const autoName = I18N.locales[resolveLanguage('auto')]?.name ?? 'English';
  const languages = [`<option value="auto" ${ui.language === 'auto' ? 'selected' : ''}>${t('general.languageAuto', { name: autoName })}</option>`]
    .concat(availableLanguages().map((l) => `<option value="${esc(l.code)}" ${ui.language === l.code ? 'selected' : ''}>${esc(l.name)}</option>`));
  // A language saved in the file that this version has no translation for.
  if (ui.language !== 'auto' && !I18N.locales[ui.language]) {
    languages.push(`<option value="${esc(ui.language)}" selected>${esc(ui.language)}</option>`);
  }

  const auto = state.autostart;
  return `<section class="card">
      <h2>${t('general.startup')}</h2>
      <label class="setting-row">
        <div><strong>${t('general.autostart')}</strong><small>${t(auto && !auto.available ? 'general.autostartMissing' : 'general.autostartHint')}</small></div>
        <input type="checkbox" data-cmd="autostart" ${auto?.enabled ? 'checked' : ''} ${auto?.available ? '' : 'disabled'}>
      </label>
    </section>
    <section class="card">
      <h2>${t('general.appearance')}</h2>
      <div class="setting-row">
        <div><strong>${t('general.theme')}</strong><small>${t('general.themeHint')}</small></div>
        <div class="segmented" role="radiogroup">
          ${THEMES.map((theme) => `<button role="radio" aria-checked="${ui.theme === theme}" class="${ui.theme === theme ? 'active' : ''}" data-cmd="theme" data-theme="${theme}">${t(`general.themes.${theme}`)}</button>`).join('')}
        </div>
      </div>
      <div class="setting-row">
        <div><strong>${t('general.language')}</strong><small>${t('general.languageHint')}</small></div>
        <select data-cmd="language" aria-label="${esc(tPlain('general.language'))}">${languages.join('')}</select>
      </div>
    </section>
    <section class="card">
      <h2>${t('general.timing')}</h2>
      ${rangeField('timing.long_press_ms', timing.long_press_ms, 250, 1500, 50, 'longPress')}
      ${rangeField('timing.double_click_ms', timing.double_click_ms, 150, 600, 25, 'doublePress')}
    </section>
    <section class="card">
      <h2>${t('general.file')}</h2>
      <p class="path">${esc(state.path)}</p>
      <div class="button-row">
        <button class="btn" data-cmd="open" data-which="config">${t('general.openNotepad')}</button>
        <button class="btn" data-cmd="open" data-which="config-folder">${t('general.showFolder')}</button>
        <button class="btn" data-cmd="open" data-which="logs">${t('general.openLogs')}</button>
      </div>
    </section>
    <section class="card">
      <h2>${t('general.about')}</h2>
      <p class="hint">${t('general.aboutText', { version: esc(state.version) })}</p>
      <div class="button-row">
        <button class="btn" data-cmd="open" data-which="project">${t('general.projectPage')}</button>
        <button class="btn" data-cmd="open" data-which="releases">${t('general.checkUpdates')}</button>
      </div>
    </section>
    <section class="card">
      <h2>${t('general.startOver')}</h2>
      <p class="hint">${t('general.startOverHint')}</p>
      <button class="btn danger" data-cmd="restore-defaults">${t('general.restore')}</button>
    </section>`;
}

const rangeField = (path, value, min, max, step, key) => `<div class="range-field">
  <div><strong>${t(`general.${key}`)}</strong><small>${t(`general.${key}Hint`)}</small></div>
  <input type="range" min="${min}" max="${max}" step="${step}" value="${esc(value)}" data-bind="${path}" data-type="int" aria-label="${esc(tPlain(`general.${key}`))}">
  <output>${t('general.ms', { value })}</output>
</div>`;

// ---------------------------------------------------------------- changes

let validateTimer = null;

/** Call after every edit. `rerender` rebuilds the page (not needed while typing). */
function changed(rerender = true) {
  if (rerender) render();
  else {
    updateChrome();
    refreshHintPreview();
  }
  clearTimeout(validateTimer);
  validateTimer = setTimeout(validate, 120);
}

async function validate() {
  const snapshot = JSON.stringify(state.config);
  let errors;
  try {
    errors = await invoke('validate_config', { config: toRust(state.config) });
  } catch (e) {
    errors = [String(e)];
  }
  if (snapshot === JSON.stringify(state.config)) {
    state.errors = errors;
    updateChrome();
  }
}

async function save() {
  if ($('#save').disabled) return;
  state.saving = true;
  updateChrome();
  try {
    await invoke('save_config', { config: toRust(state.config) });
    state.savedJson = JSON.stringify(state.config);
    state.loadError = null;
    toast(tPlain(state.daemon?.running === false ? 'toast.savedNotRunning' : 'toast.saved'));
  } catch (e) {
    toast(tPlain('toast.saveFailed', { error: e }), true);
  }
  state.saving = false;
  updateChrome();
}

let toastTimer = null;
function toast(text, error = false) {
  const el = $('#toast');
  el.textContent = text;
  el.className = `toast show ${error ? 'error' : ''}`;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { el.className = 'toast'; }, error ? 6000 : 2600);
}

/** Removes a modifier layer once nothing in it is set. */
function pruneLayer(path) {
  const parts = path.split('.');
  if (parts.length !== 4) return; // modes.i.layer.gesture
  const layerPath = parts.slice(0, 3).join('.');
  const layer = getPath(state.config, layerPath);
  if (layer && !Object.keys(layer).length) setPath(state.config, layerPath, undefined);
}

async function openPicker(index, list = 'profiles') {
  state.picker = index;
  state.pickerFor = list;
  state.appFilter = '';
  state.apps = null;
  renderPicker();
  try {
    state.apps = await invoke('running_apps');
  } catch {
    state.apps = [];
  }
  renderAppList();
}

function addSituation(effect, when) {
  effect.when ??= [];
  if (!effect.when.some((w) => w.toLowerCase() === when.toLowerCase())) effect.when.push(when);
  changed();
}

function addApp(exe) {
  const name = exe.trim();
  if (state.pickerFor === 'snippetExclude') {
    state.picker = null;
    const lower = name.toLowerCase();
    if (lower && snip.store && !snip.store.exclude.includes(lower)) {
      const next = clone(snip.store);
      next.exclude.push(lower);
      saveSnippets(next, 'snippets.savedSettings');
    } else render();
    return;
  }
  if (state.pickerFor === 'snippet') {
    state.picker = null;
    const lower = name.toLowerCase();
    if (lower && snip.draft && !snip.draft.apps.includes(lower)) snip.draft.apps.push(lower);
    render();
    snippetEdited();
    return;
  }
  if (state.pickerFor === 'effects') {
    const effect = state.config.effects[state.picker];
    state.picker = null;
    if (effect && name) addSituation(effect, `app:${name}`);
    else changed();
    return;
  }
  const list = pickerList();
  if (!name || !list) return;
  if (!list.some((e) => e.toLowerCase() === name.toLowerCase())) list.push(name);
  state.picker = null;
  changed();
}

// ---------------------------------------------------------------- commands

const commands = {
  nav: (el) => {
    if (state.page === 'snippets' && el.dataset.page !== 'snippets') {
      if (!leaveSnippetOk()) return;
      forgetSnippets();
    }
    state.page = el.dataset.page;
    state.recording = null;
    render();
    $('#main').scrollTop = 0;
  },
  save: () => save(),
  discard: () => {
    if (!state.savedJson) return;
    state.config = JSON.parse(state.savedJson);
    changed();
  },

  'select-mode': (el) => { state.modeIndex = Number(el.dataset.index); render(); },
  layer: (el) => { state.layer = el.dataset.layer; render(); },
  'add-mode': () => {
    const modes = state.config.modes;
    modes.push({
      id: uniqueId('mode'),
      name: tPlain('modes.newName'),
      rotate: { action: 'volume', step: 2, accelerate: true },
      click: { action: 'mute' },
      long_press: { action: 'cycle_mode' },
    });
    state.modeIndex = modes.length - 1;
    state.layer = '';
    changed();
    const input = document.querySelector('.editor-head input');
    input?.focus();
    input?.select();
  },
  'duplicate-mode': () => {
    const copy = clone(currentMode());
    copy.id = uniqueId(copy.id);
    copy.name = tPlain('modes.copyName', { name: copy.name });
    state.config.modes.splice(state.modeIndex + 1, 0, copy);
    state.modeIndex += 1;
    changed();
  },
  'delete-mode': () => {
    const mode = currentMode();
    if (!confirm(tPlain('modes.confirmDelete', { name: mode.name }))) return;
    state.config.modes.splice(state.modeIndex, 1);
    for (const p of state.config.profiles) p.modes = p.modes.filter((id) => id !== mode.id);
    changed();
  },

  'add-profile': () => {
    const profiles = state.config.profiles;
    const firstMode = state.config.modes[0]?.id;
    const profile = { name: tPlain('apps.newProfile'), match: { exe: [] }, modes: firstMode ? [firstMode] : [], remember_last_mode: true };
    // Insert above the catch-all entry so it isn't shadowed.
    const catchAll = profiles.findIndex((p) => p.match.exe.includes('*'));
    const at = catchAll === -1 ? profiles.length : catchAll;
    profiles.splice(at, 0, profile);
    changed();
    openPicker(at);
  },
  'delete-profile': (el) => {
    const i = Number(el.dataset.index);
    if (!confirm(tPlain('apps.confirmDelete', { name: state.config.profiles[i].name }))) return;
    state.config.profiles.splice(i, 1);
    changed();
  },
  'profile-move': (el) => {
    const i = Number(el.dataset.index);
    const j = i + Number(el.dataset.delta);
    const ps = state.config.profiles;
    [ps[i], ps[j]] = [ps[j], ps[i]];
    changed();
  },
  'remove-app': (el) => {
    state.config.profiles[Number(el.dataset.profile)].match.exe.splice(Number(el.dataset.index), 1);
    changed();
  },
  'open-picker': (el) => openPicker(Number(el.dataset.profile)),
  'close-picker': () => { state.picker = null; renderPicker(); },
  'open-remap-picker': (el) => openPicker(Number(el.dataset.index), 'remaps'),
  'open-macro-picker': (el) => openPicker(Number(el.dataset.index), 'macros'),
  'macro-remove-app': (el) => {
    state.config.macros[Number(el.dataset.index)].apps.splice(Number(el.dataset.app), 1);
    changed();
  },
  'remap-remove-app': (el) => {
    state.config.remaps[Number(el.dataset.index)].apps.splice(Number(el.dataset.app), 1);
    changed();
  },
  'add-remap': () => {
    const used = new Set(state.config.remaps.filter((r) => !r.apps.length).map((r) => r.from.toLowerCase()));
    const from = REMAP_CANDIDATES.find((k) => !used.has(k.toLowerCase())) ?? 'F24';
    state.config.remaps.push({ from, to: from === 'CapsLock' ? 'Esc' : 'MediaPlayPause', apps: [] });
    changed();
    document.querySelector(`[data-keycap="remaps.${state.config.remaps.length - 1}.from"]`)?.focus();
  },
  'delete-remap': (el) => {
    state.config.remaps.splice(Number(el.dataset.index), 1);
    changed();
  },
  'pick-app': (el) => addApp(el.dataset.exe),
  'add-manual-app': () => {
    let exe = $('#app-manual').value.trim();
    if (exe && !/\.exe$/i.test(exe)) exe += '.exe';
    addApp(exe);
  },
  'mode-move': (el) => {
    const modes = state.config.profiles[Number(el.dataset.profile)].modes;
    const i = Number(el.dataset.index);
    const j = i + Number(el.dataset.delta);
    [modes[i], modes[j]] = [modes[j], modes[i]];
    changed();
  },
  'remove-profile-mode': (el) => {
    state.config.profiles[Number(el.dataset.profile)].modes.splice(Number(el.dataset.index), 1);
    changed();
  },

  'select-macro': (el) => { state.macroIndex = Number(el.dataset.index); state.recording = null; render(); },
  'add-macro': () => {
    const macros = state.config.macros;
    macros.push({ name: uniqueMacroName(tPlain('macros.newName')), repeat: 1, apps: [], steps: [] });
    state.macroIndex = macros.length - 1;
    state.recording = null;
    changed();
    const input = document.querySelector('.editor-head input');
    input?.focus();
    input?.select();
  },
  'duplicate-macro': () => {
    const copy = clone(currentMacro());
    copy.name = uniqueMacroName(tPlain('modes.copyName', { name: copy.name }));
    delete copy.hotkey; // two macros can't share a hotkey
    state.config.macros.splice(state.macroIndex + 1, 0, copy);
    state.macroIndex += 1;
    state.recording = null;
    changed();
  },
  'delete-macro': () => {
    if (!confirm(tPlain('macros.confirmDelete', { name: currentMacro().name }))) return;
    state.config.macros.splice(state.macroIndex, 1);
    state.recording = null;
    changed();
  },
  'macro-record': () => {
    if (state.recording) {
      state.recording = null;
    } else {
      state.recording = { macro: state.macroIndex, last: 0 };
      document.activeElement?.blur();
    }
    render();
  },
  'hotkey-clear': () => { delete currentMacro().hotkey; changed(); },
  'step-remove': (el) => { currentMacro().steps.splice(Number(el.dataset.index), 1); changed(); },
  'step-move': (el) => {
    const steps = currentMacro().steps;
    const i = Number(el.dataset.index);
    const j = i + Number(el.dataset.delta);
    [steps[i], steps[j]] = [steps[j], steps[i]];
    changed();
  },
  'hint-preset': (el) => { Object.assign(state.config.hints.style, HINT_PRESETS[el.dataset.preset]); changed(); },
  'hint-reset': async () => {
    const defaults = fromRust(await invoke('default_config'));
    state.config.hints.style = defaults.hints.style;
    changed();
  },
  'select-layout': (el) => { state.layoutIndex = Number(el.dataset.index); render(); },
  'add-layout': () => {
    const layouts = state.config.lighting;
    layouts.push({ name: tPlain('lighting.newName'), effect: 1, color: '#66CCFF', colorful: false, brightness: 3, speed: 3 });
    state.layoutIndex = layouts.length - 1;
    changed();
    const input = document.querySelector('.editor-head input');
    input?.focus();
    input?.select();
  },
  'duplicate-layout': () => {
    const copy = clone(currentLayout());
    copy.name = tPlain('modes.copyName', { name: copy.name });
    state.config.lighting.splice(state.layoutIndex + 1, 0, copy);
    state.layoutIndex += 1;
    changed();
  },
  'delete-layout': () => {
    if (!confirm(tPlain('lighting.confirmDelete', { name: currentLayout().name }))) return;
    state.config.lighting.splice(state.layoutIndex, 1);
    changed();
  },
  'apply-layout': () => applyLayout(),
  'select-effect': (el) => { state.effectIndex = Number(el.dataset.index); render(); },
  'add-effect': () => {
    const slot = freeEffectSlot();
    if (slot == null) return;
    const effects = state.config.effects;
    effects.push({
      name: uniqueEffectName(tPlain('effects.newName')),
      slot, pattern: 'solid', colors: ['#66CCFF'], speed: 5, brightness: 100, duration_ms: 0,
    });
    state.effectIndex = effects.length - 1;
    changed();
    const input = document.querySelector('.editor-head input');
    input?.focus();
    input?.select();
  },
  'duplicate-effect': () => {
    const slot = freeEffectSlot();
    if (slot == null) return;
    const copy = clone(currentEffect());
    copy.name = uniqueEffectName(tPlain('modes.copyName', { name: copy.name }));
    copy.slot = slot;
    state.config.effects.splice(state.effectIndex + 1, 0, copy);
    state.effectIndex += 1;
    changed();
  },
  'delete-effect': () => {
    if (!confirm(tPlain('effects.confirmDelete', { name: currentEffect().name }))) return;
    state.config.effects.splice(state.effectIndex, 1);
    changed();
  },
  'effect-color-add': () => {
    const colors = currentEffect().colors;
    if (colors.length < EFFECT_MAX_COLORS) colors.push(colors[colors.length - 1] ?? '#FFFFFF');
    changed();
  },
  'effect-color-remove': (el) => {
    const colors = currentEffect().colors;
    if (colors.length > 1) colors.splice(Number(el.dataset.index), 1);
    changed();
  },
  'effect-restart': () => { effectPreview.start = performance.now(); },
  'effect-play': () => {
    const effect = currentEffect();
    if (effect) effectCommand('effect_play', { effect }, tPlain('effects.playing', { name: effect.name }));
  },
  'effect-stop': () => effectCommand('effect_stop', {}, tPlain('effects.stopped')),
  'effect-when-app': () => openPicker(state.effectIndex, 'effects'),
  'effect-when-remove': (el) => {
    const effect = currentEffect();
    if (!effect) return;
    effect.when.splice(Number(el.dataset.index), 1);
    changed();
  },

  theme: (el) => { state.config.ui.theme = el.dataset.theme; changed(); },
  'start-daemon': async () => {
    try {
      await invoke('start_daemon');
    } catch (e) {
      toast(String(e), true);
    }
    setTimeout(refreshDaemon, 800);
  },
  open: (el) => invoke('open_location', { which: el.dataset.which }).catch((e) => toast(String(e), true)),
  'restore-defaults': async () => {
    if (!confirm(tPlain('general.confirmRestore'))) return;
    state.config = fromRust(await invoke('default_config'));
    state.modeIndex = 0;
    state.layer = '';
    changed();
  },
};

// Commands fired by <select> / checkbox changes.
const changeCommands = {
  'step-add': (el) => {
    const defaults = {
      keys: { keys: 'Enter' }, text: { text: 'Text' }, wait: { ms: 200 }, click: { button: 'left' },
      key_down: { key: 'LeftShift' }, key_up: { key: 'LeftShift' },
      move: { x: 100, y: 0, relative: true }, scroll: { notches: 3, horizontal: false },
    };
    if (!el.value) return;
    currentMacro().steps.push({ step: el.value, ...defaults[el.value] });
    changed();
  },
  'set-action': (el) => {
    const { path, kind } = el.dataset;
    if (!el.value) {
      setPath(state.config, path, undefined);
      pruneLayer(path);
    } else {
      const defaults = ACTIONS[kind].find(([id]) => id === el.value)[1];
      setPath(state.config, path, { action: el.value, ...clone(defaults) });
      if (el.value === 'run_macro') setPath(state.config, `${path}.name`, state.config.macros[0]?.name ?? '');
    }
    changed();
  },
  'add-profile-mode': (el) => {
    if (!el.value) return;
    state.config.profiles[Number(el.dataset.profile)].modes.push(el.value);
    changed();
  },
  'toggle-every-app': (el) => {
    const match = state.config.profiles[Number(el.dataset.profile)].match;
    match.exe = el.checked ? ['*'] : [];
    changed();
  },
  'layout-effect': (el) => {
    const layout = currentLayout();
    layout.effect = Number(el.value);
    // These effects always cycle colours; the keyboard has no single-colour form.
    if (!effectInfo(layout.effect).color) layout.colorful = true;
    changed();
  },
  'layout-colorful': (el) => { currentLayout().colorful = el.checked; changed(); },
  'effect-slot': (el) => { currentEffect().slot = Number(el.value); changed(); },
  'effect-when-mode': (el) => {
    const effect = currentEffect();
    if (!effect || !el.value) return;
    addSituation(effect, `mode:${el.value}`);
  },
  'effect-when-game': (el) => {
    const effect = currentEffect();
    if (!effect || !el.value) return;
    addSituation(effect, `game:${el.value}`);
  },
  'effect-pattern': (el) => { currentEffect().pattern = el.value; changed(); },
  language: (el) => { state.config.ui.language = el.value; changed(); },
  // Applies at once, like the tray menu item; not part of Save.
  autostart: async (el) => {
    try {
      state.autostart = await invoke('set_autostart', { enabled: el.checked });
    } catch (e) {
      toast(t('toast.autostartFailed', { error: e }), true);
    }
    render();
  },
};

// ---------------------------------------------------------------- events

function coerce(el) {
  if (el.dataset.type === 'bool') return el.checked;
  if (el.dataset.type === 'int') {
    const n = parseInt(el.value, 10);
    const min = el.min === '' ? 0 : Number(el.min);
    return Number.isFinite(n) ? Math.max(min, n) : min;
  }
  return el.value;
}

document.addEventListener('input', (e) => {
  const el = e.target;
  if (el.dataset.snip && snip.draft) {
    snip.draft[el.dataset.snip] = el.value;
    snippetEdited();
    return;
  }
  if (el.hasAttribute('data-snip-filter')) {
    snip.filter = el.value;
    render();
    const search = $('.snip-search');
    search?.focus();
    search?.setSelectionRange(search.value.length, search.value.length);
    return;
  }
  if (el.id === 'app-filter') {
    state.appFilter = el.value;
    renderAppList();
    return;
  }
  if (!el.dataset.bind || el.type === 'checkbox') return;
  const macroName = /^macros\.\d+\.name$/.test(el.dataset.bind);
  if (macroName) renameMacroRefs(getPath(state.config, el.dataset.bind), el.value);
  setPath(state.config, el.dataset.bind, coerce(el));
  if (el.type === 'range') {
    el.nextElementSibling.innerHTML = el.dataset.plain ? esc(el.value) : t('general.ms', { value: el.value });
  }
  // Keep the list in step with the name being typed or the colour being picked.
  const listMatch = /^(modes|macros|lighting)\.(\d+)\.(name|color)$/.exec(el.dataset.bind);
  if (listMatch) {
    const [, list, index] = listMatch;
    const item = document.querySelector(`.mode-item[data-index="${index}"] .mode-name`);
    if (item && list !== 'lighting') item.textContent = el.value || tPlain('modes.unnamed');
    if (item && list === 'lighting') {
      const layout = state.config.lighting[Number(index)];
      item.innerHTML = `${layoutSwatch(layout)}${esc(layout.name) || t('modes.unnamed')}`;
    }
  }
  const effectMatch = /^effects\.(\d+)\.(name|colors\.\d+)$/.exec(el.dataset.bind);
  if (effectMatch) {
    const item = document.querySelector(`.mode-item[data-index="${effectMatch[1]}"] .mode-name`);
    const effect = state.config.effects[Number(effectMatch[1])];
    if (item) item.innerHTML = `${effectSwatch(effect)}${esc(effect.name) || t('modes.unnamed')}`;
  }
  changed(false);
});

document.addEventListener('change', (e) => {
  const el = e.target;
  if (el.dataset.cmd && snippetChangeCommands[el.dataset.cmd]) {
    snippetChangeCommands[el.dataset.cmd](el);
  } else if (el.dataset.cmd && changeCommands[el.dataset.cmd]) {
    changeCommands[el.dataset.cmd](el);
  } else if (el.dataset.bind && el.type === 'checkbox') {
    setPath(state.config, el.dataset.bind, el.checked);
    changed(el.dataset.bind.endsWith('.until_stopped'));
  } else if (el.dataset.special) {
    if (!el.value) return;
    setPath(state.config, el.dataset.special, el.value);
    changed();
  } else if (el.dataset.bind && el.type === 'number') {
    el.value = getPath(state.config, el.dataset.bind); // show the clamped value
  }
});

document.addEventListener('click', (e) => {
  const el = e.target.closest('[data-cmd]');
  if (!el || el.tagName === 'SELECT' || el.type === 'checkbox' || el.disabled) return;
  // Clicks inside the dialog bubble up to the overlay: only the backdrop closes it.
  if (el.classList.contains('overlay') && e.target !== el) return;
  (snippetCommands[el.dataset.cmd] ?? commands[el.dataset.cmd])?.(el);
});

// Key-combination fields: focus one, then press any key or combination.
//
// Keys are recorded in the same text form as config.toml (see keys.rs):
// named keys (Left, F5, A, Num5…), characters ("." "ç" "*"), or VK_XX for
// anything else. Characters are recorded as typed, so they work on every
// keyboard layout: the full stop is "." whether it sits next to the comma
// (US) or below Ç (Turkish Q).

// Keys whose position, not character, matters.
const KEY_BY_CODE = {
  Space: 'Space',
  NumpadMultiply: 'NumMultiply', NumpadAdd: 'NumAdd', NumpadSubtract: 'NumSubtract',
  NumpadDecimal: 'NumDecimal', NumpadDivide: 'NumDivide', NumpadEnter: 'Enter',
};
const KEY_BY_KEY = {
  ArrowLeft: 'Left', ArrowRight: 'Right', ArrowUp: 'Up', ArrowDown: 'Down',
  Enter: 'Enter', Tab: 'Tab', Escape: 'Esc', Backspace: 'Backspace', Delete: 'Delete', Insert: 'Insert',
  Home: 'Home', End: 'End', PageUp: 'PageUp', PageDown: 'PageDown', PrintScreen: 'PrintScreen',
  Pause: 'Pause', CapsLock: 'CapsLock', NumLock: 'NumLock', ScrollLock: 'ScrollLock', ContextMenu: 'Menu',
  MediaTrackNext: 'MediaNext', MediaTrackPrevious: 'MediaPrev', MediaPlayPause: 'MediaPlayPause', MediaStop: 'MediaStop',
  AudioVolumeUp: 'VolumeUp', AudioVolumeDown: 'VolumeDown', AudioVolumeMute: 'VolumeMute',
  BrowserBack: 'BrowserBack', BrowserForward: 'BrowserForward', BrowserRefresh: 'BrowserRefresh',
  BrowserStop: 'BrowserStop', BrowserSearch: 'BrowserSearch', BrowserFavorites: 'BrowserFavorites', BrowserHome: 'BrowserHome',
  LaunchMail: 'LaunchMail', LaunchMediaPlayer: 'LaunchMedia', LaunchApplication1: 'LaunchApp1', LaunchApplication2: 'LaunchApp2',
};
const MODIFIER_KEYS = ['Control', 'Shift', 'Alt', 'AltGraph', 'Meta', 'OS'];
// Modifier keys pressed on their own, by which side of the keyboard.
const MODIFIER_BY_CODE = {
  ControlLeft: 'LeftCtrl', ControlRight: 'RightCtrl', ShiftLeft: 'LeftShift', ShiftRight: 'RightShift',
  AltLeft: 'LeftAlt', AltRight: 'RightAlt', MetaLeft: 'LeftWin', MetaRight: 'RightWin', OSLeft: 'LeftWin', OSRight: 'RightWin',
};

/** "LeftCtrl" and so on for a modifier key, else null. */
function loneModifier(e) {
  if (MODIFIER_BY_CODE[e.code]) return MODIFIER_BY_CODE[e.code];
  // Some input (on-screen keyboards, remote desktop) has no key position.
  const name = { Control: 'Ctrl', Shift: 'Shift', Alt: 'Alt', AltGraph: 'Alt', Meta: 'Win', OS: 'Win' }[e.key];
  if (!name) return null;
  return `${e.location === 2 || e.key === 'AltGraph' ? 'Right' : 'Left'}${name}`;
}

/**
 * What a key field records. "single" (a macro's held key): one key,
 * modifiers included, with nothing held alongside. "source" (the key a remap
 * changes) and "target" (what it becomes): a lone modifier key or any
 * combination. Otherwise: a combination.
 */
function keyForField(e, mode) {
  const lone = loneModifier(e);
  if (mode === 'single') return lone ?? keyFromEvent(e)?.key ?? null;
  if ((mode === 'source' || mode === 'target') && lone) return lone;
  return comboFromEvent(e);
}

/** { key, isChar } for a keydown, or null for a bare modifier. */
function keyFromEvent(e) {
  if (MODIFIER_KEYS.includes(e.key)) return null;
  const altGr = e.getModifierState?.('AltGraph');
  const letter = /^Key([A-Z])$/.exec(e.code);
  const digit = /^Digit(\d)$/.exec(e.code);
  const numpad = /^Numpad(\d)$/.exec(e.code);
  // Letters and digits by position (Ctrl+Z stays Ctrl+Z on every layout),
  // unless AltGr turns them into another character (Turkish AltGr+Q = @).
  if (letter && !altGr) return { key: letter[1], isChar: false };
  if (digit && !altGr) return { key: digit[1], isChar: false };
  // Without a key position, an ASCII letter or digit is still that key.
  if (!e.code && !altGr && /^[a-z0-9]$/i.test(e.key)) return { key: e.key.toUpperCase(), isChar: false };
  if (numpad) return { key: `Num${numpad[1]}`, isChar: false };
  if (/^F([1-9]|1\d|2[0-4])$/.test(e.key)) return { key: e.key, isChar: false };
  const named = KEY_BY_CODE[e.code] ?? KEY_BY_KEY[e.key];
  if (named) return { key: named, isChar: false };
  // Any other key that types one character: . , ; ' ç ö * + …
  if ([...e.key].length === 1) return { key: e.key, isChar: true };
  // Anything else (dead keys, unusual keys): its raw virtual-key code.
  if (e.keyCode > 0 && e.keyCode < 0xff) {
    return { key: `VK_${e.keyCode.toString(16).toUpperCase().padStart(2, '0')}`, isChar: false };
  }
  return null;
}

function comboFromEvent(e) {
  const pressed = keyFromEvent(e);
  if (!pressed) return null;
  const { key, isChar } = pressed;
  // A character already says which Shift / AltGr state typed it (Shift+. is
  // ":" on some layouts), so those aren't recorded separately.
  const altGr = isChar && e.getModifierState?.('AltGraph');
  const mods = [];
  if (e.ctrlKey && !altGr) mods.push('Ctrl');
  if (e.shiftKey && !isChar) mods.push('Shift');
  if (e.altKey && !altGr) mods.push('Alt');
  if (e.metaKey) mods.push('Win');
  return [...mods, key].join('+');
}

document.addEventListener('keydown', (e) => {
  if (state.recording) {
    recordKey(e);
    return;
  }
  const el = document.activeElement;
  if (el?.dataset?.keycap) {
    const plainTab = e.key === 'Tab' && !e.ctrlKey && !e.altKey && !e.metaKey;
    if (plainTab) return; // keep keyboard navigation working
    e.preventDefault();
    const combo = keyForField(e, el.dataset.keymode);
    if (!combo) return;
    setPath(state.config, el.dataset.keycap, combo);
    el.value = combo;
    el.classList.remove('flash');
    void el.offsetWidth; // restart the animation
    el.classList.add('flash');
    if (/^macros\.\d+\.hotkey$/.test(el.dataset.keycap)) {
      // The list and the clear button show the hotkey too.
      changed();
      document.querySelector(`[data-keycap="${el.dataset.keycap}"]`)?.focus();
      return;
    }
    changed(false);
    return;
  }
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
    e.preventDefault();
    save();
  } else if (e.key === 'Escape' && state.picker != null) {
    commands['close-picker']();
  } else if (e.key === 'Enter' && e.target.id === 'app-manual') {
    commands['add-manual-app']();
  }
});

// ---------------------------------------------------------------- start

async function refreshDaemon() {
  const daemon = await invoke('daemon_state').catch(() => null);
  if (JSON.stringify(daemon) === JSON.stringify(state.daemon)) return;
  state.daemon = daemon;
  if (state.config) updateChrome();
}

/** Closing the window must not silently throw away edits. */
function guardUnsavedChanges() {
  window.__TAURI__?.window?.getCurrentWindow?.().onCloseRequested((event) => {
    if (state.config && isDirty() && !confirm(tPlain('banners.confirmClose'))) event.preventDefault();
  });
}

async function init() {
  applyAppearance();
  try {
    const loaded = await invoke('load_config');
    state.config = fromRust(loaded.config);
    state.path = loaded.path;
    state.loadError = loaded.error;
    state.autostart = await invoke('get_autostart').catch(() => null);
    state.version = await invoke('app_version').catch(() => '');
    await refreshDaemon();
    setInterval(refreshDaemon, 3000);
    guardUnsavedChanges();
    // A broken file counts as unsaved, so one Save replaces it with what is shown.
    state.savedJson = loaded.error ? '' : JSON.stringify(state.config);
    render();
    validate();
  } catch (e) {
    $('#main').innerHTML = `<div class="banner error">${t('banners.loadFailed', { error: e })}</div>`;
  }
}

if (invoke) init();
