# Plan: Add .ebv Support to Syntax Highlighter

## Goal
Extend the VS Code syntax highlighter to support `.ebv` (Embedded Brief) files using the e-brief-logo.svg icon.

## Current State
- Syntax highlighter in `syntax-highlighter/` supports `.bv` and `.rbv`
- Icon exists at `/home/randozart/Desktop/Projects/brief-compiler/assets/e-brief-logo.svg`
- `.ebv` uses same syntax as `.bv` (with additional `trg` keyword)

## Steps

### 1. Copy e-brief logo to syntax-highlighter
- Copy `/home/randozart/Desktop/Projects/brief-compiler/assets/e-brief-logo.svg` to `/home/randozart/Desktop/Projects/brief-compiler/syntax-highlighter/images/e-brief-logo.svg`

### 2. Update package.json
Add new language entry for `.ebv` and grammar entry reusing brief grammar.

### 3. Add `trg` keyword to brief.tmLanguage.json
Add `trg` to the keywords section to be colored the same as `let`/`const`.

### 4. Rebuild the extension
Run `vsce package` in the syntax-highlighter directory.

### 5. Reinstall extension
Copy updated extension to VSCode/VSCodium extensions folder.

## Files to Modify
- `syntax-highlighter/package.json`
- `syntax-highlighter/syntaxes/brief.tmLanguage.json`

## Files to Create
- `syntax-highlighter/images/e-brief-logo.svg`