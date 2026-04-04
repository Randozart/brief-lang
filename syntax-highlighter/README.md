# Brief Language - VSCode Extension

Syntax highlighting, folding, bracket colorization, and intelligent editing for Brief (`.bv`) and Rendered Brief (`.rbv`) files.

## Features

- **Syntax Highlighting**: Keywords, types, operators, strings, numbers, comments
- **Bracket Colorization**: Matching brackets highlighted in different colors
- **Auto-closing**: Automatically closes `{}`, `[]`, `()`, and `""`
- **Folding**: Fold the PATH ANALYSIS annotation block
- **Auto-indentation**: Smart indentation for Brief code blocks
- **Themes**: Dark and Light themes included

## Installation

### Option 1: Link Locally (Recommended for Development)

```bash
# Create symlink to extensions folder
ln -s "$(pwd)" ~/.vscode/extensions/brief-language
# Or for VScodium:
ln -s "$(pwd)" ~/.config/VSCodium/User/extensions/brief-language
```

### Option 2: Package as .vsix

```bash
npm install -g @vscode/vsce
vsce package
code --install-extension brief-language-0.1.0.vsix
```

## Color Scheme

| Element | Color | Style |
|---------|-------|-------|
| `txn` | Purple | Bold |
| `rct` | Yellow | Bold |
| `term`/`escape` | Gold | Bold |
| `defn`/`let`/`sig` | Blue | Normal |
| Types | Cyan | Normal |
| Custom Types | Green | Normal |
| `true`/`false` | Blue | Bold |
| Strings | Orange | Normal |
| Comments | Green | Italic |
| Annotations | Bright Green | Bold |
| `&` ownership | Purple | Bold |
| `@` prior state | Yellow | Italic |

## Keyboard Shortcuts

| Action | Shortcut |
|--------|----------|
| Fold all | `Ctrl+Shift+[` |
| Unfold all | `Ctrl+Shift+]` |
| Fold region | `Ctrl+Shift+Ctrl+[` |
| Format document | `Alt+Shift+F` |

## Settings

Enable bracket colorization:
```json
{
  "editor.bracketPairColorization.enabled": true,
  "editor.guides.bracketPairs": true
}
```

## File Extension

Files with `.bv` extension are automatically recognized.

## Development

To modify the grammar:
1. Edit `syntaxes/brief.tmLanguage.json`
2. Reload VSCode window

To modify themes:
1. Edit `themes/brief-dark.json` or `themes/brief-light.json`
2. Reload window

## Extension Structure

```
brief-language/
├── package.json              # Extension manifest
├── language-configuration.json # Brackets, indentation rules
├── settings.json             # Default settings
├── syntaxes/
│   └── brief.tmLanguage.json # TextMate grammar
├── themes/
│   ├── brief-dark.json       # Dark theme
│   └── brief-light.json      # Light theme
└── README.md
```
