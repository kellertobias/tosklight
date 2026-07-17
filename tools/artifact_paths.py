"""Resolve the repository-owned artifact layout for direct Python tooling."""

from __future__ import annotations

import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def _layout() -> dict[str, str]:
    entries: dict[str, str] = {}
    for line in (ROOT / "tools" / "artifact-layout.conf").read_text(encoding="utf-8").splitlines():
        if line and not line.startswith("#"):
            key, value = line.split("=", 1)
            entries[key] = value
    return entries


def artifact_path(environment: str, key: str) -> Path:
    if environment in os.environ:
        configured = os.environ[environment]
        if not configured:
            raise ValueError(f"{environment} cannot be empty")
        return Path(configured).expanduser().resolve()
    configured_root = os.environ.get("LIGHT_ARTIFACTS_DIR", str(ROOT / ".artifacts"))
    if not configured_root:
        raise ValueError("LIGHT_ARTIFACTS_DIR cannot be empty")
    root = Path(configured_root).expanduser().resolve()
    return root / _layout()[key]
