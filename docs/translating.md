# Translating

The program has two parts with text in them, and each has its own list of
strings. English is the reference: anything a translation leaves out is
shown in English, so a partial translation is fine.

Users pick the language under **Settings → General → Language**, or keep
**Same as Windows**, which uses the Windows display language when a
translation for it exists. The choice is stored as `language` in the `[ui]`
section of `config.toml` (`"auto"` or a code such as `"tr"`).

Use the two-letter [ISO 639-1](https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes)
code (`tr`, `de`, `pt`). A regional code (`pt-BR`) works too; it is tried
before the base language.

## 1. The Settings window

1. Copy `crates/knob-settings/ui/locales/en.js` to `<code>.js`, for example
   `tr.js`.
2. Change the first line to your code and the language's own name:
   ```js
   registerLocale('tr', 'Türkçe', {
   ```
3. Translate the values. Keep the keys, the `{placeholders}` and the simple
   HTML (`<strong>`, `<kbd>`, `<code>`) as they are.
4. Plurals use the forms of
   [`Intl.PluralRules`](https://developer.mozilla.org/docs/Web/JavaScript/Reference/Global_Objects/Intl/PluralRules)
   for your language (`zero`, `one`, `two`, `few`, `many`, `other`):
   ```js
   problems: { one: '{count} sorun', other: '{count} sorun' },
   ```
5. Add your file to `crates/knob-settings/ui/index.html`, after `en.js`:
   ```html
   <script src="locales/tr.js"></script>
   ```

## 2. The tray menu and popups

1. In `crates/ak820-custom-knob/src/i18n.rs`, copy the `fn en` function and
   rename it to your code (`fn tr_tr` if the name clashes with something).
2. Translate the strings. Return `None` for anything you skip:
   ```rust
   fn tr_tr(text: Text) -> Option<&'static str> {
       use Text::*;
       Some(match text {
           MenuQuit => "Çıkış",
           // …
           _ => return None, // falls back to English
       })
   }
   ```
3. Add it to `LANGUAGES`:
   ```rust
   const LANGUAGES: &[(&str, Language)] = &[("en", en), ("tr", tr_tr)];
   ```

## Not translated

Mode and app-entry names are the user's own text and are never translated.
The built-in default modes ("Volume", "Scroll", "Media") are written into
the user's `config.toml` in English the first time the program starts.
