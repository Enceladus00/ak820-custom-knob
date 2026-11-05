'use strict';

// Translation layer for the Settings window.
//
// Every language is one file in ui/locales/ that calls registerLocale(), plus
// a <script> line in index.html. English (en.js) is the reference: any key a
// language doesn't have falls back to English. See docs/translating.md.

const I18N = {
  locales: {},
  active: 'en',
  fallback: 'en',
};

/** Called by each file in ui/locales/. `name` is the language's own name, e.g. "Deutsch". */
function registerLocale(code, name, strings) {
  I18N.locales[code] = { name, strings };
}

function availableLanguages() {
  return Object.entries(I18N.locales)
    .map(([code, locale]) => ({ code, name: locale.name }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

/** "auto" → the Windows display language if we have it; otherwise the given code, or English. */
function resolveLanguage(preference) {
  const candidates = preference && preference !== 'auto'
    ? [preference]
    : (navigator.languages?.length ? navigator.languages : [navigator.language]);
  for (const tag of candidates) {
    if (!tag) continue;
    if (I18N.locales[tag]) return tag;
    const base = tag.split(/[-_]/)[0].toLowerCase();
    if (I18N.locales[base]) return base;
  }
  return I18N.fallback;
}

function setLanguage(preference) {
  I18N.active = resolveLanguage(preference);
  document.documentElement.lang = I18N.active;
}

function lookup(code, key) {
  return key.split('.').reduce((node, part) => (node == null ? undefined : node[part]), I18N.locales[code]?.strings);
}

function translate(key, params, escapeParams) {
  let value = lookup(I18N.active, key) ?? lookup(I18N.fallback, key);
  if (value == null) {
    console.warn(`missing translation: ${key}`);
    return key;
  }
  // Plural forms: { one: '…', other: '…' }, chosen by params.count.
  if (typeof value === 'object') {
    const form = new Intl.PluralRules(I18N.active).select(params.count ?? 0);
    value = value[form] ?? value.other;
  }
  return value.replace(/\{(\w+)\}/g, (match, name) => {
    if (!(name in params)) return match;
    const text = String(params[name]);
    return escapeParams
      ? text.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]))
      : text;
  });
}

/** For HTML: translations may contain simple markup; parameters are escaped. */
function t(key, params = {}) {
  return translate(key, params, true);
}

/** For plain text (confirm dialogs, attributes set through the DOM): nothing is escaped. */
function tPlain(key, params = {}) {
  return translate(key, params, false);
}
