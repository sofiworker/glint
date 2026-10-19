"""Distribution invariants: privacy boundary, license choice/provenance and archive integrity."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location("packager", Path(__file__).with_name("package_windows.py"))
packager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packager)


class DistributionInvariants(unittest.TestCase):
    def test_source_snapshot_excludes_local_images_configuration_and_caches(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            allowed = ["Cargo.toml", "Cargo.lock", "LICENSE", "crates/app/src/main.rs",
                       "prototype/index.html", "prototype/icons/copy.svg", ".cargo/config.toml",
                       "tools/package_windows.py", "licenses/reference/MIT.txt"]
            private = [".local/config.toml", ".local/pins.session", ".local/capture.png",
                       "prototype/.qa/screen.png", "crates/app/target/private.rs",
                       "tools/__pycache__/cache.py", ".git/config", "crates/app/.env", "secret.toml"]
            for name in allowed + private:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(name, encoding="utf-8")
            actual = {p.relative_to(root).as_posix() for p in packager.source_files(root)}
            self.assertEqual(actual, set(allowed))

    def test_known_license_paths_preserve_obligations_and_reject_unknown_conjunctions(self):
        self.assertEqual(packager.choose_license("GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0"), ["GPL-3.0-only"])
        self.assertEqual(set(packager.choose_license("(MIT OR Apache-2.0) AND Unicode-3.0")), {"MIT", "Unicode-3.0"})
        self.assertEqual(set(packager.choose_license("Apache-2.0 AND MIT")), {"MIT", "Apache-2.0"})
        self.assertEqual(packager.choose_license("Apache-2.0/MIT"), ["MIT"])
        self.assertEqual(packager.choose_license("Apache-2.0 WITH LLVM-exception"), ["Apache-2.0 WITH LLVM-exception"])
        for expression in ["", "LicenseRef-Unknown", "MIT AND LicenseRef-Unknown", "MIT OR", "(MIT", "MIT Apache-2.0"]:
            with self.assertRaises(ValueError, msg=expression):
                packager.choose_license(expression)

    def test_changed_or_escaping_license_material_cannot_be_packaged(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "licenses").mkdir()
            text = root / "licenses/terms.txt"
            text.write_text("original terms", encoding="utf-8")
            entry = {"path": "licenses/terms.txt", "sha256": packager.digest(text)}
            manifest = {"references": [entry], "supplements": []}
            path = root / "licenses/provenance.json"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            packager.checked_provenance(root)
            text.write_text("changed terms", encoding="utf-8")
            with self.assertRaises(ValueError):
                packager.checked_provenance(root)
            entry["path"] = "../outside.txt"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaises(ValueError):
                packager.checked_provenance(root)

    def test_archives_preserve_unicode_bytes_and_do_not_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            (source / "sub").mkdir(parents=True)
            (source / "sub/\u8bb8\u53ef.txt").write_bytes(b"GPL-3.0-only\n")
            (source / "Cargo.lock").write_bytes(bytes(range(256)))
            archive = root / "source.zip"
            packager.zip_tree(source, archive)
            before = packager.digest(archive)
            with zipfile.ZipFile(archive) as result:
                self.assertIsNone(result.testzip())
                self.assertEqual(result.read("sub/\u8bb8\u53ef.txt"), b"GPL-3.0-only\n")
                self.assertEqual(result.read("Cargo.lock"), bytes(range(256)))
                self.assertTrue(all(i.date_time == (1980, 1, 1, 0, 0, 0) for i in result.infolist()))
            with self.assertRaises(FileExistsError):
                packager.zip_tree(source, archive)
            self.assertEqual(packager.digest(archive), before)


if __name__ == "__main__":
    unittest.main()
