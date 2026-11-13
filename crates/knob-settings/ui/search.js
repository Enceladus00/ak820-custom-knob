'use strict';

// Find a setting: the search box above the page list.
//
// Modelled on how the Windows Sticky Notes app searches its notes:
// - results update as you type, and the page shows only the matches;
// - the matching letters in each result are marked;
// - the number of results is announced to screen readers;
// - empty results get their own message;
// - Escape (or picking a result, or another page) resets the search.
// The matching itself is ours: every word must appear somewhere in the
// result's name or description, ignoring case and accents.
//
// Searched: page names, the settings on the General and Mouse & hints pages,
// and the names of modes, app profiles, remaps, macros, lighting layouts and
// effects. Snippets are not: their text stays encrypted until their page is open.

const find = {
  query: '',
  active: 0, // index of the highlighted result
  announceTimer: null,
};

const findInput = () => document.getElementById('find');
const findActive = () => find.query.trim() !== '';

/**
 * Lower case without accents ("Ünlü" → "unlu", "İ" and "ı" → "i"), plus a
 * map from each folded position back to the original text, so matches can
 * be marked in the text as written.
 */
function fold(text) {
  const source = String(text ?? '');
  let out = '';
  const map = [];
  for (let i = 0; i < source.length;) {
    const ch = String.fromCodePoint(source.codePointAt(i));
    let folded = ch.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();
    if (folded === 'ı') folded = 'i';
    for (let k = 0; k < folded.length; k++) map.push(i);
    out += folded;
    i += ch.length;
  }
  map.push(source.length);
  return { text: out, map };
}

const findTerms = (query) => fold(query).text.split(/\s+/).filter(Boolean);

/** Every place the terms occur in `text`, as merged [start, end) ranges of the original text. */
function hitRanges(text, terms) {
  const { text: folded, map } = fold(text);
  const ranges = [];
  for (const term of terms) {
    for (let at = folded.indexOf(term); at !== -1; at = folded.indexOf(term, at + 1)) {
      ranges.push([map[at], map[at + term.length]]);
    }
  }
  ranges.sort((a, b) => a[0] - b[0]);
  const merged = [];
  for (const r of ranges) {
    const last = merged[merged.length - 1];
    if (last && r[0] <= last[1]) last[1] = Math.max(last[1], r[1]);
    else merged.push([...r]);
  }
  return merged;
}

/** Escaped HTML with <mark> around the ranges. */
function markHits(text, ranges) {
  let html = '';
  let at = 0;
  for (const [start, end] of ranges) {
    html += `${esc(text.slice(at, start))}<mark>${esc(text.slice(start, end))}</mark>`;
    at = end;
  }
  return html + esc(text.slice(at));
}

/** Everything the search can find, built fresh from the current settings. */
function findIndex() {
  const config = state.config;
  const items = [];
  const seen = new Set();
  const add = (page, title, detail = '', target = {}) => {
    title = String(title ?? '').trim();
    detail = String(detail ?? '').trim();
    const key = `${page}\n${title}\n${detail}`;
    if (!title || seen.has(key)) return;
    seen.add(key);
    items.push({ page, title, detail, ...target });
  };
  const textOf = (html) => {
    const box = document.createElement('template');
    box.innerHTML = html;
    return box.content.textContent.replace(/\s+/g, ' ').trim();
  };
  const unnamed = tPlain('modes.unnamed');

  for (const p of PAGES) add(p.id, tPlain(`pages.${p.id}.title`), textOf(t(`pages.${p.id}.sub`)));

  config.modes.forEach((m, i) => add('modes', m.name || unnamed, textOf(modeUsage(m.id)), { select: { modeIndex: i } }));
  config.profiles.forEach((p, i) => add('apps', p.name || unnamed,
    p.match.exe.map((exe) => (exe === '*' ? tPlain('apps.everyApp') : exe)).join(', '),
    { locate: () => document.querySelectorAll('#main .card.profile')[i] }));
  config.remaps.forEach((r, i) => add('remap', `${r.from || '?'} → ${r.to || '?'}`,
    r.apps.length ? r.apps.join(', ') : tPlain('apps.everyApp'),
    { locate: () => document.querySelectorAll('#main .remap-row')[i] }));
  config.macros.forEach((m, i) => add('macros', m.name || unnamed, m.hotkey || '', { select: { macroIndex: i } }));
  config.lighting.forEach((l, i) => add('lighting', l.name || unnamed, tPlain(`lighting.effects.${l.effect}`), { select: { layoutIndex: i } }));
  (config.effects ?? []).forEach((e, i) => add('effects', e.name || unnamed,
    e.when.map(situationLabel).join(', '), { select: { effectIndex: i } }));

  // Settings rows, read from the pages themselves so the search never falls
  // behind them. Only pages whose templates have no side effects.
  for (const [page, template] of [['general', generalPage], ['controls', controlsPage]]) {
    const box = document.createElement('template');
    try {
      box.innerHTML = template();
    } catch (e) {
      console.warn(`search: skipped the ${page} page`, e);
      continue;
    }
    box.content.querySelectorAll('.setting-row, .range-field').forEach((row, k) => {
      const name = row.querySelector('strong')?.textContent;
      const section = row.closest('section')?.querySelector('h2')?.textContent ?? '';
      const hint = row.querySelector('small')?.textContent ?? '';
      add(page, name, [section, hint].filter(Boolean).join(' · '),
        { locate: () => document.querySelectorAll('#main .setting-row, #main .range-field')[k] });
    });
    box.content.querySelectorAll('section h2').forEach((h, k) => add(page, h.textContent, '',
      { locate: () => document.querySelectorAll('#main section h2')[k]?.closest('section') }));
  }
  return items;
}

/** Results for the query: every term must match the name or the description. Best first. */
function findResults(query) {
  const terms = findTerms(query);
  if (!terms.length) return [];
  const order = (page) => PAGES.findIndex((p) => p.id === page);
  const results = [];
  findIndex().forEach((item, position) => {
    const title = fold(item.title).text;
    const detail = fold(item.detail).text;
    if (!terms.every((term) => title.includes(term) || detail.includes(term))) return;
    const inTitle = terms.filter((term) => title.includes(term)).length;
    const startsTitle = title.startsWith(terms[0]) || title.includes(` ${terms[0]}`);
    results.push({
      ...item,
      score: (terms.length - inTitle) * 2 + (startsTitle ? 0 : 1),
      position,
      order: order(item.page),
      ranges: { title: hitRanges(item.title, terms), detail: hitRanges(item.detail, terms) },
    });
  });
  return results.sort((a, b) => a.score - b.score || a.order - b.order || a.position - b.position);
}

function findPage(results) {
  if (!results.length) {
    return `<div class="card muted find-empty">
      <strong>${t('find.none', { query: find.query.trim() })}</strong>
      <p class="hint">${t('find.noneHint')}</p>
    </div>`;
  }
  find.active = Math.min(find.active, results.length - 1);
  const hits = results.map((hit, k) => {
    const page = PAGES.find((p) => p.id === hit.page);
    return `<button class="find-hit ${k === find.active ? 'active' : ''}" id="find-hit-${k}" role="option"
        aria-selected="${k === find.active}" data-cmd="find-open" data-index="${k}" tabindex="-1">
      <span class="find-text">
        <span class="find-title">${markHits(hit.title, hit.ranges.title)}</span>
        ${hit.detail ? `<span class="find-detail">${markHits(hit.detail, hit.ranges.detail)}</span>` : ''}
      </span>
      <span class="find-page">${icon(page.icon)}${t(`pages.${page.id}.label`)}</span>
    </button>`;
  }).join('');
  return `<div class="find-results" id="find-results" role="listbox" aria-label="${esc(tPlain('find.resultsLabel'))}">${hits}</div>
    <p class="hint">${t('find.keys')}</p>`;
}

/** Called by render() while a search is active, in place of the current page. */
function renderFind() {
  const results = findResults(find.query);
  find.results = results;
  $('#title').innerHTML = t('find.title');
  $('#subtitle').innerHTML = t('find.count', { count: results.length, query: find.query.trim() });
  $('#main').innerHTML = findPage(results);
  findInput()?.setAttribute('aria-activedescendant', results.length ? `find-hit-${find.active}` : '');
  announceFind(results.length);
}

/** Tells screen readers how many results there are, once typing pauses. */
function announceFind(count) {
  clearTimeout(find.announceTimer);
  find.announceTimer = setTimeout(() => {
    const status = document.getElementById('find-status');
    if (status) status.textContent = findActive() ? tPlain('find.announce', { count }) : '';
  }, 400);
}

function resetFind() {
  find.query = '';
  find.active = 0;
  const input = findInput();
  if (input) {
    input.value = '';
    input.removeAttribute('aria-activedescendant');
  }
  announceFind(0);
}

function openFindHit(hit) {
  if (!hit) return;
  if (state.page === 'snippets' && hit.page !== 'snippets') {
    if (!leaveSnippetOk()) return;
    forgetSnippets();
  }
  state.page = hit.page;
  state.recording = null;
  Object.assign(state, hit.select ?? {});
  resetFind();
  render();
  $('#main').scrollTop = 0;
  const target = hit.locate?.();
  if (target) {
    target.scrollIntoView({ block: 'center' });
    target.classList.remove('find-flash');
    void target.offsetWidth; // restart the animation
    target.classList.add('find-flash');
  }
}

function moveFind(delta) {
  const count = find.results?.length ?? 0;
  if (!count) return;
  find.active = (find.active + delta + count) % count;
  renderFind();
  document.getElementById(`find-hit-${find.active}`)?.scrollIntoView({ block: 'nearest' });
}

commands['find-open'] = (el) => openFindHit(find.results?.[Number(el.dataset.index)]);

// Going to another page ends the search.
const navWithoutFind = commands.nav;
commands.nav = (el) => {
  resetFind();
  navWithoutFind(el);
};

document.addEventListener('input', (e) => {
  if (e.target.id !== 'find') return;
  find.query = e.target.value;
  find.active = 0;
  render();
});

document.addEventListener('keydown', (e) => {
  if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === 'f' && !state.recording) {
    e.preventDefault();
    findInput()?.focus();
    findInput()?.select();
    return;
  }
  if (e.target.id !== 'find') return;
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    moveFind(e.key === 'ArrowDown' ? 1 : -1);
  } else if (e.key === 'Enter') {
    e.preventDefault();
    openFindHit(find.results?.[find.active]);
  } else if (e.key === 'Escape') {
    e.preventDefault();
    if (findActive()) {
      resetFind();
      render();
    } else {
      e.target.blur();
    }
  }
});
