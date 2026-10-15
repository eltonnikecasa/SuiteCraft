# Translating SuiteCraft

SuiteCraft keeps human-editable translations in the root [`i18n/`](i18n/) directory. The current catalogs are `en.json` (English and the fallback catalog) and `pt-BR.json` (Brazilian Portuguese).

Catalogs are flat JSON objects. Keys are stable technical identifiers and values are text shown to users. Application names, GitHub URLs, versions, filesystem paths, and other technical identifiers must remain unchanged.

## Adding a language

1. Copy `i18n/en.json` to `i18n/<language-code>.json`. Use a BCP 47 code such as `es`, `fr`, or `de-DE`.
2. Translate values only. Preserve every key, JSON syntax, placeholders such as `{name}`, and meaningful punctuation.
3. Register the catalog and language code in `src/i18n.ts`, then add it to the selector in `src/App.tsx`.
4. Add a localized label for the language to every catalog.
5. Run the checks below and request review from a fluent speaker.

Do not translate application or product names unless the upstream project has an official localized name. Keep UI text concise enough for narrow buttons and responsive layouts. Prefer natural language over literal word-for-word translation.

## Validation

```bash
npm test
npm run build
CARGO_BUILD_JOBS=15 cargo test --manifest-path src-tauri/Cargo.toml -j 15
```

Frontend tests verify catalog completeness, interpolation, locale detection, and English fallback. Rust tests verify preference persistence and invalid values. Generated build output is not committed; both JSON catalogs are source files.

During review, switch languages from **Settings → Language**, check long labels at a narrow window width, exercise confirmations and errors, and restart the app to confirm that the preference is retained.
