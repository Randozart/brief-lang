# Plan: Add .ebv Support to Syntax Highlighter

## Goal
Extend the VS Code syntax highlighter to support `.ebv` (Embedded Briev) files using the e-briev-logo.svg icon.

## Current State
- Syntax highlighter in `syntax-highlighter/` supports `.bv` and `.rbv`
- Icon exists at `/home/randozart/Desktop/Projects/briev-compiler/assets/e-briev-logo.svg`
- `.ebv` uses same syntax as `.bv` (with additional `trg` keyword)

## Steps

### 1. Copy e-briev logo to syntax-highlighter
- Copy `/home/randozart/Desktop/Projects/briev-compiler/assets/e-briev-logo.svg` to `/home/randozart/Desktop/Projects/briev-compiler/syntax-highlighter/images/e-briev-logo.svg`

### 2. Update package.json
Add new language entry for `.ebv` and grammar entry reusing briev grammar.

### 3. Add `trg` keyword to briev.tmLanguage.json
Add `trg` to the keywords section to be colored the same as `let`/`const`.

### 4. Rebuild the extension
Run `vsce package` in the syntax-highlighter directory.

### 5. Reinstall extension
Copy updated extension to VSCode/VSCodium extensions folder.

## Files to Modify
- `syntax-highlighter/package.json`
- `syntax-highlighter/syntaxes/briev.tmLanguage.json`

## Files to Create
- `syntax-highlighter/images/e-briev-logo.svg`