#!/usr/bin/env python3
"""Build an isolated Windows development distribution, including corresponding source.

Python 3.11+, Cargo/Rust and the MSVC/Windows SDK toolchain are prerequisites.
No installation, user configuration, Git staging or publishing is performed.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-pc-windows-msvc"
ROOT_FILES = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "rustfmt.toml",
              "README.md", "AGENTS.md", "LICENSE", "THIRD-PARTY-NOTICES.md", ".gitignore"}
SOURCE_DIRS = {"crates", "prototype", "tools", "licenses", "assets", ".cargo"}
EXCLUDED = {".local", ".qa", ".git", "target", "__pycache__", "node_modules"}
SUFFIXES = {".rs", ".slint", ".svg", ".toml", ".md", ".txt", ".license",
            ".py", ".js", ".html", ".css", ".json", ".png", ".ico", ".ttf"}
ALLOWED_LICENSES = {"MIT", "Apache-2.0", "GPL-3.0-only", "Unicode-3.0", "BSD-3-Clause",
                    "BSD-2-Clause", "0BSD", "Zlib", "BSL-1.0", "ISC", "MPL-2.0",
                    "Unlicense", "CC0-1.0", "Apache-2.0 WITH LLVM-exception"}
PREFERENCE = ["MIT", "Apache-2.0", "BSD-3-Clause", "BSD-2-Clause", "0BSD", "Zlib",
              "BSL-1.0", "ISC", "CC0-1.0", "Unlicense", "Unicode-3.0", "MPL-2.0",
              "Apache-2.0 WITH LLVM-exception", "GPL-3.0-only"]


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def run(args: list[str], *, cwd: Path, log: Path, env: dict[str, str] | None = None) -> str:
    log.parent.mkdir(parents=True, exist_ok=True)
    # Compiler/vendor progress is written live to a local log; command stdout
    # remains separate so metadata/tree output can be parsed without diagnostics.
    with log.open("w", encoding="utf-8") as errors:
        result = subprocess.run(args, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=errors, text=True, encoding="utf-8")
    with log.open("a", encoding="utf-8") as combined:
        combined.write("\n" + result.stdout)
    if result.returncode:
        raise RuntimeError(f"Command failed ({result.returncode}): {' '.join(args)}; see {log}")
    return result.stdout


def source_files(root: Path) -> list[Path]:
    paths = [root / name for name in ROOT_FILES if (root / name).is_file()]
    for name in sorted(SOURCE_DIRS):
        base = root / name
        if base.is_symlink():
            raise ValueError(f"Source directory is a symlink: {name}")
        if not base.exists():
            continue
        for directory, dirs, files in os.walk(base, followlinks=False):
            dirs[:] = [d for d in dirs if d not in EXCLUDED]
            for child in dirs:
                if (Path(directory) / child).is_symlink():
                    raise ValueError("Source symlink directories are unsupported")
            for child in files:
                path = Path(directory) / child
                if path.suffix in SUFFIXES or child in {"LICENSE", "COPYING", "NOTICE"}:
                    paths.append(path)
    for path in paths:
        if path.is_symlink() or not path.resolve().is_relative_to(root.resolve()):
            raise ValueError("Source file escapes the project")
    return sorted(set(paths), key=lambda p: p.relative_to(root).as_posix())


def choose_license(expression: str) -> list[str]:
    """Select explicit known SPDX paths; preserve conjunctions and reject unknown grants."""
    normalized = expression.replace("/", " OR ")
    tokens = re.findall(r"\(|\)|[^\s()]+", normalized)
    index = 0

    def atom() -> list[str] | None:
        nonlocal index
        if index >= len(tokens):
            raise ValueError("Incomplete license expression")
        token = tokens[index]
        index += 1
        if token == "(":
            result = disjunction()
            if index >= len(tokens) or tokens[index] != ")":
                raise ValueError("Unbalanced license expression")
            index += 1
            return result
        if token in {"AND", "OR", ")", "WITH"}:
            raise ValueError("Invalid license expression")
        if index < len(tokens) and tokens[index] == "WITH":
            index += 1
            if index >= len(tokens):
                raise ValueError("Missing license exception")
            token += " WITH " + tokens[index]
            index += 1
        return [token] if token in ALLOWED_LICENSES else None

    def conjunction() -> list[str] | None:
        nonlocal index
        result = atom()
        while index < len(tokens) and tokens[index] == "AND":
            index += 1
            other = atom()
            result = sorted(set(result + other)) if result is not None and other is not None else None
        return result

    def disjunction() -> list[str] | None:
        nonlocal index
        choices = [conjunction()]
        while index < len(tokens) and tokens[index] == "OR":
            index += 1
            choices.append(conjunction())
        known = [c for c in choices if c is not None]
        return min(known, key=lambda c: (max(PREFERENCE.index(x) for x in c), len(c))) if known else None

    result = disjunction()
    if index != len(tokens) or result is None:
        raise ValueError(f"License expression needs review: {expression}")
    return result


def checked_provenance(root: Path) -> dict:
    provenance = json.loads((root / "licenses/provenance.json").read_text(encoding="utf-8"))
    for item in provenance["references"] + provenance["supplements"]:
        path = root / item["path"]
        if not path.resolve().is_relative_to((root / "licenses").resolve()) or path.is_symlink():
            raise ValueError("License provenance path escapes licenses")
        if digest(path) != item["sha256"]:
            raise ValueError(f"License text changed: {item['path']}")
    return provenance


def pe_imports(path: Path) -> list[str]:
    """Read PE32+ import names, without executing the packaged file."""
    data = path.read_bytes()
    if data[:2] != b"MZ":
        raise ValueError("Executable is not PE")
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("Invalid PE signature")
    machine, count = struct.unpack_from("<HH", data, pe + 4)
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    optional = pe + 24
    if machine != 0x8664 or struct.unpack_from("<H", data, optional)[0] != 0x20B:
        raise ValueError("Expected a Windows x64 PE32+ executable")
    sections = optional + optional_size
    def offset(rva: int) -> int:
        for i in range(count):
            virtual_size, address, raw_size, raw = struct.unpack_from("<IIII", data, sections + i * 40 + 8)
            if address <= rva < address + max(virtual_size, raw_size):
                return raw + rva - address
        raise ValueError("PE RVA is outside a section")
    imports_rva = struct.unpack_from("<I", data, optional + 112 + 8)[0]
    if not imports_rva:
        return []
    cursor = offset(imports_rva)
    names = []
    while any(struct.unpack_from("<IIIII", data, cursor)):
        name_rva = struct.unpack_from("<I", data, cursor + 12)[0]
        start = offset(name_rva)
        end = data.index(0, start)
        names.append(data[start:end].decode("ascii"))
        cursor += 20
    return sorted(set(names), key=str.lower)


def inventory(source: Path, portable: Path, tree: str, provenance: dict) -> list[dict]:
    graph = {tuple(match.groups()) for line in tree.splitlines()
             if (match := re.match(r"([^ ]+) v([^ ]+)", line))}
    lock = tomllib.loads((source / "Cargo.lock").read_text(encoding="utf-8"))
    checksums = {(p["name"], p["version"]): p["checksum"] for p in lock["package"] if "source" in p}
    licenses = portable / "licenses"
    shutil.copytree(source / "licenses", licenses)
    packages = []
    for base in sorted((source / "vendor").iterdir()):
        if not base.is_dir():
            continue
        package = tomllib.loads((base / "Cargo.toml").read_text(encoding="utf-8"))["package"]
        key = (package["name"], package["version"])
        checksum = json.loads((base / ".cargo-checksum.json").read_text(encoding="utf-8"))["package"]
        if checksum != checksums.pop(key, None):
            raise ValueError(f"Vendored package checksum does not match Cargo.lock: {key}")
        expression = package.get("license")
        if not expression:
            raise ValueError(f"Missing license expression: {key}")
        selected = choose_license(expression)
        for choice in selected:
            for identifier in choice.split(" WITH "):
                if not (licenses / "reference" / f"{identifier}.txt").is_file():
                    raise ValueError(f"Missing license terms: {identifier}")
        files = []
        copyright_lines = []
        for path in sorted(base.rglob("*")):
            if path.is_symlink():
                raise ValueError("Vendored symlink is unsupported")
            if not path.is_file():
                continue
            relative = path.relative_to(base)
            if any(word in path.name.lower() for word in ["license", "licence", "copying", "copyright", "notice"]) or path.name == "REUSE.toml":
                dest = licenses / "crates" / base.name / relative
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, dest)
                files.append(dest.relative_to(portable).as_posix())
            if path.suffix.lower() in {".rs", ".slint", ".svg", ".c", ".h", ".cpp", ".hpp", ".md", ".txt", ".license"}:
                # Full source is also shipped; these are searchable notice excerpts,
                # not a replacement for the original file or a copyright audit.
                with path.open("r", encoding="utf-8", errors="replace") as stream:
                    for number, line in enumerate(stream, 1):
                        if number > 100:
                            break
                        if "copyright" in line.casefold() or "SPDX-FileCopyrightText" in line:
                            copyright_lines.append(f"{relative.as_posix()}:{number}: {line.rstrip()}")
        notice = licenses / "crates" / base.name / "SOURCE-ATTRIBUTION.txt"
        notice.parent.mkdir(parents=True, exist_ok=True)
        notice.write_text("Authors declared in Cargo metadata:\n" + "\n".join(package.get("authors", [])) +
                          "\n\nSource header excerpts (full original sources accompany this build):\n" +
                          "\n".join(copyright_lines) + "\n", encoding="utf-8")
        supplements = [s["path"] for s in provenance["supplements"] if "@".join(key) in s["packages"]]
        packages.append({"name": key[0], "version": key[1], "source_checksum": checksum,
                         "declared_license": expression, "selected_license": selected,
                         "windows_build_dependency": key in graph, "repository": package.get("repository"),
                         "authors": package.get("authors", []), "original_notices": files,
                         "supplements": supplements,
                         "terms_basis": "Original crate texts plus pinned upstream supplements and SPDX reference terms; see licenses/README.md"})
    if checksums:
        raise ValueError("Vendor directory is missing locked packages")
    write_json(portable / "DEPENDENCIES.json", packages)
    lines = ["# Dependency inventory", "", "GPL-3.0-only is selected for Slint. Windows build entries include build-time/proc-macro dependencies, not just linked runtime code.",
             "Other entries are source-only material retained by standard Cargo vendoring for locked resolution. They do not establish support for those platforms/features.",
             "Original license/notice files, attribution excerpts and reference terms are in licenses/. Full original sources are in corresponding-source.zip.", "",
             "| Package | Version | Selected path | Windows normal/build graph |", "|---|---|---|---|"]
    lines.extend(f"| {p['name']} | {p['version']} | {' AND '.join(p['selected_license'])} | {'yes' if p['windows_build_dependency'] else 'source only'} |" for p in packages)
    (portable / "DEPENDENCIES.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return packages


def zip_tree(base: Path, archive: Path, *, skip: set[str] | None = None) -> None:
    with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED, compresslevel=6, allowZip64=True) as output:
        for directory, dirs, files in os.walk(base, followlinks=False):
            dirs[:] = sorted(d for d in dirs if d not in (skip or set()))
            for name in sorted(files):
                path = Path(directory) / name
                if path.is_symlink():
                    raise ValueError("Archive symlinks are unsupported")
                relative = path.relative_to(base).as_posix()
                info = zipfile.ZipInfo(relative, (1980, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_STORED if path.suffix == ".zip" else zipfile.ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                with path.open("rb") as stream, output.open(info, "w", force_zip64=True) as dest:
                    shutil.copyfileobj(stream, dest, 1024 * 1024)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="A new, empty output directory; never overwritten")
    args = parser.parse_args()
    if sys.platform != "win32":
        raise ValueError("This packager currently verifies Windows x64 MSVC only")
    if any(os.environ.get(name) for name in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"]):
        raise ValueError("Use an environment without custom Rust flags for a recorded baseline build")
    output = args.output.resolve()
    if output.exists():
        raise ValueError("Output directory already exists; use a new path")
    if output == ROOT or output in ROOT.parents:
        raise ValueError("Output must not contain the project")
    if output.is_relative_to(ROOT) and output.relative_to(ROOT).parts[0] != ".local":
        raise ValueError("Generated project-local distributions must be inside ignored .local/")
    provenance = checked_provenance(ROOT)
    paths = source_files(ROOT)
    baseline = {p.relative_to(ROOT).as_posix(): digest(p) for p in paths}
    output.mkdir(parents=True)
    logs = output / "logs"
    compiler = run(["rustc", "-Vv"], cwd=ROOT, log=logs / "rustc.log")
    if f"host: {TARGET}" not in compiler:
        raise ValueError(f"Compiler host must be {TARGET}")
    release = re.search(r"^release: (\S+)$", compiler, re.M).group(1)
    active = run(["rustup", "show", "active-toolchain"], cwd=ROOT, log=logs / "toolchain.log").split()[0]
    source = output / "source"
    for path in paths:
        dest = source / path.relative_to(ROOT)
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, dest)
    # Pin the recorded compiler in the distributed source, without editing the user's toolchain file.
    (source / "rust-toolchain.toml").write_text(f'[toolchain]\nchannel = "{release}"\nprofile = "minimal"\ncomponents = ["rustfmt", "clippy"]\n', encoding="utf-8")
    print("Vendoring locked dependency sources...", flush=True)
    run(["cargo", "vendor", "--locked", "--versioned-dirs", "--respect-source-config", str(source / "vendor")], cwd=ROOT, log=logs / "vendor.log")
    config = source / ".cargo/config.toml"
    with config.open("a", encoding="utf-8") as stream:
        stream.write('\n[source.crates-io]\nreplace-with = "vendored-sources"\n[source.vendored-sources]\ndirectory = "vendor"\n')
    env = dict(os.environ)
    env["RUSTUP_TOOLCHAIN"] = active
    env["CARGO_TARGET_DIR"] = str(output / "build")
    tree = run(["cargo", "tree", "--frozen", "--target", TARGET, "--features", "glint-app/native-shell", "--edges", "normal,build", "--prefix", "none", "--format", "{p}"], cwd=source, log=logs / "tree.log", env=env)
    portable = output / "portable"
    portable.mkdir()
    packages = inventory(source, portable, tree, provenance)
    sysroot = Path(run(["rustc", "--print", "sysroot"], cwd=ROOT, log=logs / "sysroot.log").strip())
    rust_notices = sysroot / "share/doc/rust"
    if not (rust_notices / "COPYRIGHT-library.html").is_file() or not (rust_notices / "licenses").is_dir():
        raise ValueError("Matching Rust library copyright/terms are missing; install rust-docs for this toolchain before packaging")
    shutil.copyfile(rust_notices / "COPYRIGHT-library.html", portable / "licenses/RUST-COPYRIGHT-library.html")
    shutil.copytree(rust_notices / "licenses", portable / "licenses/rust-toolchain-terms")
    print(f"Collected {len(packages)} vendored packages; {sum(p['windows_build_dependency'] for p in packages)} third-party packages in the Windows build graph.", flush=True)
    print("Building only from staged source and vendor with --frozen...", flush=True)
    command = ["cargo", "build", "--frozen", "--release", "--target", TARGET, "-p", "glint-app", "--features", "native-shell"]
    run(command, cwd=source, log=logs / "build.log", env=env)
    executable = output / "build" / TARGET / "release/glint-app.exe"
    imports = pe_imports(executable)
    shutil.copyfile(executable, portable / "glint-app.exe")
    for name in ["LICENSE", "THIRD-PARTY-NOTICES.md"]:
        shutil.copyfile(source / name, portable / name)
    shutil.copytree(source / "crates/glint-ui/ui/widgets", portable / "licenses/glint-modified-slint-widgets")
    shutil.copytree(source / "assets/fonts", portable / "licenses/fonts")
    (source / "BUILDING.md").write_text(
        "# Rebuild this Windows development build\n\nInstall Python3.11+ (for packaging), Rust " + release +
        " with the x86_64-pc-windows-msvc target, Visual Studio C++ build tools and Windows SDK. Toolchain/SDK installation is a prerequisite and can require a network connection.\n\n"
        "After installing the toolchain, the included Cargo.lock and unmodified vendor/ permit the application build without registry/network access:\n\n"
        "```powershell\n" + " ".join(command) + "\n```\n\n"
        "The source pins the compiler and uses the software renderer. The original project .cargo aliases are retained. "
        "No .local/, user configuration, screenshots, history, credentials or Git metadata is included. "
        "The vendor directory also contains unused-platform sources for Cargo resolution, not a claim of support. "
        "This is a reproducible procedure, not a promise of bit-identical PE files across machines/SDKs. "
        "M0-M5 environment, physical-input, clean-machine/runtime and release gates remain incomplete.\n",
        encoding="utf-8")
    manifest = {"kind": "development-build", "version": tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"],
                "target": TARGET, "features": ["native-shell"], "renderer": "Slint software",
                "compiler": compiler, "command": command, "source_inputs_sha256": baseline,
                "executable_sha256": digest(portable / "glint-app.exe"), "pe_imports": imports,
                "runtime_prerequisites": ["Windows x64", "Microsoft Visual C++ 2015-2022 Redistributable (x64)"] if any(n.lower().startswith("vcruntime") for n in imports) else ["Windows x64"],
                "vendored_packages": len(packages), "windows_third_party_build_packages": sum(p["windows_build_dependency"] for p in packages),
                "milestones_complete": False, "clean_machine_verified": False}
    write_json(portable / "BUILD.json", manifest)
    (portable / "README.txt").write_text(
        "Glint - Windows x64 development build\n\nRun glint-app.exe. First launch opens onboarding; closing settings returns to the tray. Exit from the tray menu.\n"
        "Settings use the normal per-user AppData location; this package is unpackable, not an isolated-config portable mode. "
        "For isolated evaluation: glint-app.exe --settings --config-dir <new-directory>\n\n"
        "Screenshot/edit/copy/save/pin are native Rust + Slint; no WebView. OCR, scrolling capture and recording are deferred. "
        "Physical interaction, mixed DPI/multiple monitors/RDP/HDR/no3D/4K/long-running resources and clean-machine runtime tests remain incomplete. "
        "No installer, signing, automatic updates or release acceptance is claimed. Windows and any required runtime DLLs must already be installed. "
        "This MSVC build requires the Microsoft Visual C++ 2015-2022 Redistributable (x64), including VCRUNTIME140.dll. "
        "Use Microsoft's official supported downloads: https://learn.microsoft.com/cpp/windows/latest-supported-vc-redist . No Windows/CRT DLL is copied from the development computer.\n\n"
        "PE imports (not a transitive/runtime compatibility proof):\n" + "\n".join(imports) +
        "\n\ncorresponding-source.zip includes project source, pinned toolchain/build instructions and Cargo-vendored dependencies. "
        "Keep it, LICENSE and the third-party materials with this development distribution. DEPENDENCIES.md/json distinguish the build graph from source-only packages. "
        "Read source/BUILDING.md inside the source ZIP to rebuild. GPL-3.0-only; no warranty.\n", encoding="utf-8")
    if baseline != {p.relative_to(ROOT).as_posix(): digest(p) for p in source_files(ROOT)}:
        raise ValueError("Project sources changed while packaging; retain logs and rerun into a new output directory")
    staged = {p.relative_to(source).as_posix(): digest(p) for p in source.rglob("*") if p.is_file()}
    write_json(source / "SOURCE-SHA256.json", staged)
    print("Writing source and development distribution archives...", flush=True)
    zip_tree(source, portable / "corresponding-source.zip")
    shipped = {p.relative_to(portable).as_posix(): digest(p) for p in portable.rglob("*") if p.is_file()}
    write_json(portable / "SHA256.json", shipped)
    archive = output / f"glint-{manifest['version']}-dev-windows-x64.zip"
    zip_tree(portable, archive)
    (output / "SHA256SUMS.txt").write_text(f"{digest(archive)}  {archive.name}\n", encoding="ascii")
    write_json(output / "RESULT.json", {"archive": archive.name, "sha256": digest(archive), "status": "built-from-frozen-vendored-source", "release_ready": False})
    print(f"Created {archive} ({archive.stat().st_size / 1024 / 1024:.1f} MiB)", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, RuntimeError, OSError, subprocess.SubprocessError) as error:
        print(f"Packaging stopped: {error}", file=sys.stderr)
        sys.exit(1)
