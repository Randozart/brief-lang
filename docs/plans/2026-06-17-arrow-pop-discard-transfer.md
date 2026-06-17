# Remaining Work: Arrow Complete + String Escape + Officina Exit

**Date:** 2026-06-17
**Session:** Backend Completeness — Arrow Pop/Discard/Transfer + Lexer Fixes

## Work Item 1: Fix `\0` Escape in String Literals

### Current State
`lexer.rs:352-359` has a comment: "For simplicity, just return the string
slice without unescaping for now." All escape sequences (`\n`, `\t`, `\0`,
`\\`, `\"`) are left as raw characters. A string `"hello\0world"` becomes
the raw bytes `h,e,l,l,o,\,0,w,o,r,l,d`.

### Fix
Add the same escape handling already implemented for char literals to string
literals: `\n` → newline, `\t` → tab, `\\` → backslash, `\"` → quote,
`\0` → null, `\u{...}` → unicode.

## Work Item 2: Add `#!exit` to officina.bv

### Current State
Officina runs until killed. The compiler warns about no exit path.

### Fix
Add `#!exit keypress == '\x03';` at the top of `officina.bv`. The `\x03`
(Ctrl+C) char is sent when the user presses Ctrl+C, causing the program to
exit cleanly.

## Work Item 3: Arrow Pop Implementation

For `value <- &list` or `let x = <- &list`:
1. Load list header `{ data_ptr, len }`
2. Calculate target slot: `data_ptr[--len]`
3. Load the element value
4. Allocate new buffer with `len` (shorter by 1)
5. Copy elements (excluding the popped one)
6. Store back updated header
7. Return the popped value

## Work Item 4: ArrowDiscard Implementation

For `<- &list[index]`:
1. Load list header
2. Remove the element at the specified index
3. Allocate new buffer with compacted elements
4. Store back updated header
5. Return Void

## Work Item 5: ArrowTransfer Implementation

For `&dest <- &source` (or `&dest <- &source; filter`):
1. Load both list headers
2. Move matching elements from source to dest
3. Store both updated headers back
4. Return Void

## Verification
1. `cargo test --lib` — all tests pass
2. Officina compiles with `#!exit`
3. Generated IR shows proper arrow pop/discard/transfer code
