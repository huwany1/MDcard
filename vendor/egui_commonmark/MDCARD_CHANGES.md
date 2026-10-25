# MDcard vendor changes

Upstream: https://github.com/lampsitter/egui_commonmark, crate egui_commonmark 0.22.0.
The original MIT OR Apache-2.0 permissions, copyright and both upstream license texts remain intact.

This checkout contains local adaptations used by MDcard: source range/preview integration, layout and media behavior, and integration with the application's controlled renderer. The source files in this directory are the authoritative modified source. This source import retains them byte-for-byte from local baseline a73f367; it does not claim they are an untouched upstream release or relicense the upstream implementation to AGPL.

The upstream portions retain their MIT OR Apache-2.0 permissions. MDcard-owned additions and
modifications are Copyright (c) 2026 MDcard contributors and are provided under
AGPL-3.0-only, as declared in the repository's COPYRIGHT and LICENSE. This grant
does not replace the upstream copyright, license texts or attribution
requirements. The upstream 0.22.0 crate and this checkout's source files identify
the original and modified versions for comparison.

The modified package's Cargo license expression is
`(MIT OR Apache-2.0) AND AGPL-3.0-only` to describe both portions. LICENSE-AGPL
contains the full terms for the MDcard-owned changes. The application linking
this modified component is AGPL-3.0-only. Distributors must retain LICENSE-MIT,
LICENSE-APACHE, LICENSE-AGPL and this change notice alongside the project's
AGPL declaration. Review changes against the upstream 0.22.0 crate
before proposing an update.
