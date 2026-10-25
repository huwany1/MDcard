# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""License texts and scope notices that must accompany new MDcard packages."""
from pathlib import Path
import shutil

PROJECT_LICENSE_FILES = (
    "LICENSE",
    "COPYRIGHT",
    "docs/LICENSING.md",
    "docs/THIRD_PARTY.md",
    "vendor/egui_commonmark/LICENSE-MIT",
    "vendor/egui_commonmark/LICENSE-APACHE",
    "vendor/egui_commonmark/LICENSE-AGPL",
    "vendor/egui_commonmark/MDCARD_CHANGES.md",
    "vendor/egui_commonmark/PATCHES.txt",
)


def copy_project_licenses(root: Path, stage: Path) -> None:
    stage = stage.resolve()
    legacy = stage / "docs/open-source/LEGACY_MIT_LICENSE.txt"
    if not legacy.resolve().is_relative_to(stage):
        raise ValueError("Legacy license cleanup must stay inside the package stage")
    legacy.unlink(missing_ok=True)
    for relative in PROJECT_LICENSE_FILES:
        destination = stage / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / relative, destination)
