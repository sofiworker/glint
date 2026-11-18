#!/usr/bin/env python3
"""Fetch the optional, pinned Windows media component beside a Glint executable.

This is a local developer/user setup command, not a distribution packager.
It does not modify PATH, install codecs, require admin, or run during app startup.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import urllib.request
import zipfile

VERSION = "9.0.2"
URL = f"https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-{VERSION}-essentials_build.zip"
SHA256 = "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba"
SOURCE = "https://github.com/FFmpeg/FFmpeg/commit/946fcce07b"


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def setup(archive: Path, directory: Path) -> None:
    if digest(archive) != SHA256:
        raise ValueError("Media archive SHA-256 mismatch; nothing was installed")
    with zipfile.ZipFile(archive) as bundle:
        names = {Path(info.filename).name: info for info in bundle.infolist()
                 if Path(info.filename).name in {"ffmpeg.exe", "ffprobe.exe", "LICENSE", "README.txt"}}
        if set(names) != {"ffmpeg.exe", "ffprobe.exe", "LICENSE", "README.txt"}:
            raise ValueError("Media archive is missing executables, license or provenance")
        # Verify every destination before writing any of them. No zip paths are extracted.
        for name, info in names.items():
            destination = directory / name
            if destination.exists():
                expected = hashlib.sha256(bundle.read(info)).hexdigest()
                if not destination.is_file() or digest(destination) != expected:
                    raise ValueError(f"Destination differs; select another empty directory: {destination}")
        directory.mkdir(parents=True, exist_ok=True)
        for name, info in names.items():
            destination = directory / name
            if not destination.exists():
                with bundle.open(info) as source, destination.open("xb") as target:
                    shutil.copyfileobj(source, target)
    manifest = {"version": VERSION, "url": URL, "archive_sha256": SHA256,
                "license": "GPL-3.0-or-later", "source": SOURCE,
                "files": {name: digest(directory / name) for name in sorted(names)}}
    manifest_path = directory / "component.json"
    text = json.dumps(manifest, ensure_ascii=False, indent=2) + "\n"
    if manifest_path.exists() and manifest_path.read_text(encoding="utf-8") != text:
        raise ValueError(f"Component manifest differs: {manifest_path}")
    manifest_path.write_text(text, encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, default=Path("target/debug/media"),
                        help="media/ directory beside glint-app.exe")
    parser.add_argument("--archive", type=Path, help="reuse a downloaded, checksum-verified archive")
    args = parser.parse_args()
    directory = args.directory.resolve()
    with tempfile.TemporaryDirectory(prefix="glint-media-setup-") as temporary:
        archive = args.archive
        if archive is None:
            archive = Path(temporary) / "ffmpeg.zip"
            print(f"Downloading FFmpeg {VERSION} from {URL}", flush=True)
            with urllib.request.urlopen(URL, timeout=30) as source, archive.open("xb") as target:
                shutil.copyfileobj(source, target)
        setup(archive, directory)
    print(f"Verified media component: {directory}. Restart Glint to detect it.")


if __name__ == "__main__":
    main()
