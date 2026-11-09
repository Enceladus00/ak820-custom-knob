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
  { id: 'macros', icon: 'macro' },
  { id: 'controls', icon: 'pointer' },
  { id: 'lighting', icon: 'light' },
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
    ['scroll', { lines: 1, horizontal: false, accelerate: true }],
    ['zoom', {}],
    ['keys', { cw: 'Right', ccw: 'Left' }],
    ['passthrough', {}],
    ['none', {}],
  ],
  press: [
    ['mute', {}],
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
  dial: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M5.6 18.4A9 9 0 1 1 18.4 18.4"/><circle cx="12" cy="12" r="4"/><path d="M14.5 9.5 17 7"/></svg>',
  apps: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7.5" height="7.5" rx="2"/><rect x="13.5" y="3" width="7.5" height="7.5" rx="2"/><rect x="3" y="13.5" width="7.5" height="7.5" rx="2"/><rect x="13.5" y="13.5" width="7.5" height="7.5" rx="2"/></svg>',
  gear: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M2 12h3M19 12h3M4.9 19.1 7 17M17 7l2.1-2.1"/></svg>',
  light: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18h6M10 21h4"/><path d="M12 3a6 6 0 0 0-3.8 10.6c.7.6 1.1 1.3 1.2 2.4h5.2c.1-1.1.5-1.8 1.2-2.4A6 6 0 0 0 12 3Z"/></svg>',
  macro: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 10h.01M11 10h.01M15 10h.01M8 14h8"/></svg>',
  pointer: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 3l6.5 17 2.4-6.9L21 10.7 5 3Z"/></svg>',
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
  macroIndex: 0,
  recording: null, // { macro, last } while a macro is being recorded
  applying: false,
  autostart: null, // { available, enabled }; null until known
  picker: null, // index of the profile the app picker adds to
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
  const { mode, profile, macro, ...rest } = raw;
  const config = { ...rest, modes: mode, profiles: profile, macros: macro ?? [] };
  for (const p of config.profiles) {
    if (typeof p.match.exe === 'string') p.match.exe = [p.match.exe];
  }
  return config;
}

function toRust(config) {
  const { modes, profiles, macros, ...rest } = config;
  return { ...rest, mode: modes, profile: profiles, macro: macros };
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
  const pages = { modes: modesPage, apps: appsPage, macros: macrosPage, controls: controlsPage, lighting: lightingPage, general: generalPage };
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
  const taken = new Set(state.config.profiles[state.picker]?.match.exe.map((e) => e.toLowerCase()) ?? []);
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

// ---------- macros page

const currentMacro = () => state.config.macros[state.macroIndex];
const STEP_TYPES = ['keys', 'text', 'wait', 'click'];

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
      <div><strong>${t('macros.repeat')}</strong></div>
      ${numberParam(`macros.${i}.repeat`, item.repeat, 1, 100, t('macros.times'))}
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
    </section>`;
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
  else updateChrome();
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
    toast(tPlain('toast.saved'));
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

async function openPicker(profileIndex) {
  state.picker = profileIndex;
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

function addApp(exe) {
  const name = exe.trim();
  if (!name || state.picker == null) return;
  const list = state.config.profiles[state.picker].match.exe;
  if (!list.some((e) => e.toLowerCase() === name.toLowerCase())) list.push(name);
  state.picker = null;
  changed();
}

// ---------------------------------------------------------------- commands

const commands = {
  nav: (el) => { state.page = el.dataset.page; state.recording = null; render(); $('#main').scrollTop = 0; },
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
    macros.push({ name: uniqueMacroName(tPlain('macros.newName')), repeat: 1, steps: [] });
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

  theme: (el) => { state.config.ui.theme = el.dataset.theme; changed(); },
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
    const defaults = { keys: { keys: 'Enter' }, text: { text: 'Text' }, wait: { ms: 200 }, click: { button: 'left' } };
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
  changed(false);
});

document.addEventListener('change', (e) => {
  const el = e.target;
  if (el.dataset.cmd && changeCommands[el.dataset.cmd]) {
    changeCommands[el.dataset.cmd](el);
  } else if (el.dataset.bind && el.type === 'checkbox') {
    setPath(state.config, el.dataset.bind, el.checked);
    changed(false);
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
  commands[el.dataset.cmd]?.(el);
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
    const combo = comboFromEvent(e);
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

async function init() {
  applyAppearance();
  try {
    const loaded = await invoke('load_config');
    state.config = fromRust(loaded.config);
    state.path = loaded.path;
    state.loadError = loaded.error;
    state.autostart = await invoke('get_autostart').catch(() => null);
    // A broken file counts as unsaved, so one Save replaces it with what is shown.
    state.savedJson = loaded.error ? '' : JSON.stringify(state.config);
    render();
    validate();
  } catch (e) {
    $('#main').innerHTML = `<div class="banner error">${t('banners.loadFailed', { error: e })}</div>`;
  }
}

if (invoke) init();
