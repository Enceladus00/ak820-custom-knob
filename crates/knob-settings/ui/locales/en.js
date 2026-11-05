'use strict';

// English: the reference language. Copy this file to add a translation;
// see docs/translating.md. Text may contain simple HTML (<strong>, <kbd>,
// <code>); {name} placeholders are filled in by the app.

registerLocale('en', 'English', {
  app: {
    name: 'AK820 Knob',
    subtitle: 'Settings',
    footer: 'Changes apply as soon as you save.',
  },

  pages: {
    modes: { label: 'Modes', title: 'Modes', sub: 'What the knob does. Each mode is one set of actions; a long press switches modes.' },
    apps: { label: 'Apps', title: 'Apps', sub: 'Which modes the knob has in which app.' },
    general: { label: 'General', title: 'General', sub: 'Appearance, timing, files and defaults.' },
  },

  status: {
    saving: 'Saving…',
    saved: 'Saved',
    unsaved: 'Unsaved changes',
    problems: { one: '{count} problem to fix', other: '{count} problems to fix' },
  },

  buttons: {
    save: 'Save',
    saveTitle: 'Save (Ctrl+S)',
    discard: 'Discard',
    duplicate: 'Duplicate',
    delete: 'Delete',
    add: 'Add',
    close: 'Close',
  },

  banners: {
    loadError: '<strong>Your config file has an error, so the defaults are shown.</strong> Saving replaces the file. {error}',
    problems: '<strong>Fix these before saving:</strong>',
    loadFailed: 'Could not load the settings: {error}',
  },

  gestures: {
    rotate: { label: 'Turn', hint: 'Runs once per click of the knob, in either direction.' },
    click: { label: 'Press', hint: 'A short press.' },
    long_press: { label: 'Long press', hint: 'Holding the knob down.' },
    double_click: { label: 'Double press', hint: 'Two quick presses. Makes single presses in this mode wait a moment.' },
  },

  layers: {
    none: 'No modifier',
    ctrl: 'Ctrl',
    shift: 'Shift',
    alt: 'Alt',
    win: 'Win',
    hintNone: 'What the knob does in this mode.',
    hint: 'What the knob does while you hold <kbd>{key}</kbd>. Anything left as “Same as without modifier” keeps the normal behaviour.',
  },

  actions: {
    unset: 'Not used',
    inherit: 'Same as without modifier',
    rotate: {
      volume: 'System volume',
      app_volume: 'Volume of the app in front',
      scroll: 'Scroll',
      zoom: 'Zoom (Ctrl + mouse wheel)',
      keys: 'Press keys',
      passthrough: 'Normal knob (volume up / down)',
      none: 'Do nothing',
    },
    press: {
      mute: 'Mute / unmute',
      cycle_mode: 'Switch to the next mode',
      keys: 'Press keys',
      mouse_click: 'Mouse click',
      passthrough: 'Normal knob (mute)',
      none: 'Do nothing',
    },
  },

  params: {
    stepUnit: '% per click',
    linesUnit: 'lines per click',
    sideways: 'Sideways',
    accelerate: 'Faster when spinning fast',
    clockwise: 'Clockwise',
    counterClockwise: 'Counter-clockwise',
    keys: 'Keys',
    keycapTitle: 'Click here, then press any key or key combination',
    specialKey: 'Special key…',
    mouse: { left: 'Left button', middle: 'Middle button', right: 'Right button' },
  },

  specialKeys: {
    MediaPlayPause: 'Play / pause',
    MediaNext: 'Next track',
    MediaPrev: 'Previous track',
    MediaStop: 'Stop media',
    VolumeMute: 'Mute',
    BrowserBack: 'Browser back',
    BrowserForward: 'Browser forward',
    BrowserRefresh: 'Browser refresh',
    PrintScreen: 'Print Screen',
  },

  modes: {
    name: 'Mode name',
    unnamed: 'Unnamed',
    newName: 'New mode',
    copyName: '{name} copy',
    newMode: 'New mode',
    usedIn: 'Used in {names}',
    unused: 'Not used by any app',
    empty: 'No modes yet. Create one to get started.',
    confirmDelete: 'Delete the mode “{name}”? Apps that use it will lose it.',
  },

  apps: {
    lead: 'When an app comes to the front, the <strong>first</strong> matching entry from the top decides which modes the knob has. A long press switches between them in the order shown.',
    apps: 'Apps',
    modes: 'Modes',
    name: 'Name',
    everyApp: 'Every app',
    noApps: 'No apps yet',
    noModes: 'No modes',
    addApp: 'Add app',
    addMode: '+ Add mode…',
    remember: 'Remember the last used mode for these apps',
    newProfile: 'New apps',
    addProfile: 'Add apps with their own modes',
    order: 'Checked in this order',
    earlier: 'Check earlier',
    later: 'Check later',
    modeEarlier: 'Earlier',
    modeLater: 'Later',
    remove: 'Remove',
    delete: 'Delete',
    confirmDelete: 'Delete “{name}”?',
    shadowing: 'Entries below this one are never used, because this one matches every app. Move it to the bottom.',
    noAppsWarning: 'Add at least one app, otherwise this entry is never used.',
    picker: {
      title: 'Add an app',
      hint: 'Pick an app that is open right now, or type its exe name (for example <code>Resolve.exe</code>).',
      search: 'Search open apps…',
      loading: 'Looking for open apps…',
      none: 'No matching open app. You can type the exe name below.',
      manual: 'Name.exe',
    },
  },

  general: {
    appearance: 'Appearance',
    theme: 'Theme',
    themeHint: 'Colours of this window.',
    themes: { system: 'Same as Windows', light: 'Light', dark: 'Dark' },
    language: 'Language',
    languageHint: 'Used by Settings and by the knob’s tray menu and popups.',
    languageAuto: 'Same as Windows ({name})',
    timing: 'Timing',
    longPress: 'Long press',
    longPressHint: 'How long to hold the knob for a long press.',
    doublePress: 'Double press',
    doublePressHint: 'How quickly the second press must follow. Only matters in modes that use double press.',
    ms: '{value} ms',
    file: 'Configuration file',
    openNotepad: 'Open in Notepad',
    showFolder: 'Show folder',
    openLogs: 'Open log folder',
    startOver: 'Start over',
    startOverHint: 'Replaces everything with the built-in defaults. Nothing changes until you save.',
    restore: 'Restore defaults',
    confirmRestore: 'Replace all settings with the defaults? Nothing changes until you save.',
  },

  toast: {
    saved: 'Saved. The knob uses the new settings now.',
    saveFailed: 'Could not save: {error}',
  },
});
