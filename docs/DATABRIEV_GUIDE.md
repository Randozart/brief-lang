# Data Briev Guide (Archived)

**This document is obsolete.** The Data Briev format has been redesigned.

See the new specification at `docs/architecture/data-briev.md`.

Key changes:
- `" "` quotes are opt-in via parser flag, not the default
- `,` replaced by `;` as universal terminator
- `.dbvs` extension removed — schema lives inline in `.dbv` or is imported
- `>` replaces both `@` and `#` as the single non-data prefix symbol
- All values are bare tokens by default; `" "` only when explicitly needed

This file is retained for historical reference only. Do not reference in new code.
