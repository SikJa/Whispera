"""Focused settings migration tests. Uses temporary files, never user settings."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("connector", Path(__file__).with_name("connect-claude-wsl.py"))
connector = importlib.util.module_from_spec(spec)
spec.loader.exec_module(connector)


class ConnectionTests(unittest.TestCase):
    def test_switch_preserves_other_settings_and_previous_bars(self):
        with tempfile.TemporaryDirectory(prefix="whispera-wsl-") as temporary:
            root = Path(temporary)
            win, wsl, manifest = root / "win.json", root / "wsl.json", root / "bridge.json"
            original = {"type": "command", "command": "echo original"}
            old, new = {"command": "windows collector"}, {"command": "wsl collector"}
            win.write_text(json.dumps({"theme": "light", "statusLine": old}))
            wsl.write_text(json.dumps({"permissions": {"allow": ["Read"]}}))
            manifest.write_text(json.dumps({"settings": "windows-path", "previous": original, "installed": old}))
            (root / "claude.json").write_text('{"oldAccount":true}')
            with patch.object(connector, "linux_path", return_value=win):
                self.assertTrue(connector.connect(root, wsl, new, "wsl-unc-path"))
            self.assertEqual(json.loads(win.read_text()), {"theme": "light", "statusLine": original})
            self.assertEqual(json.loads(wsl.read_text()), {"permissions": {"allow": ["Read"]}, "statusLine": new})
            saved = json.loads(manifest.read_text())
            self.assertIsNone(saved["previous"])
            self.assertEqual(saved["settings"], "wsl-unc-path")
            self.assertFalse((root / "claude.json").exists())
            self.assertFalse(connector.connect(root, wsl, new, "wsl-unc-path"))
            self.assertEqual(len(list(root.glob("*.whispera-backup-*.json"))), 3)
            before = win.read_bytes()
            with patch.object(connector, "linux_path", return_value=win):
                with self.assertRaises(ValueError):
                    connector.connect(root, wsl, {"command": "another"}, "new-unc")
            self.assertEqual(win.read_bytes(), before)

    def test_partial_failure_restores_original_bytes(self):
        with tempfile.TemporaryDirectory(prefix="whispera-wsl-") as temporary:
            root = Path(temporary)
            settings = root / "settings.json"
            before = b'{"theme":"dark", "statusLine":{"command":"original"}}\n'
            settings.write_bytes(before)
            write = connector.atomic
            def fail_manifest(path, raw):
                if path.name == "bridge.json":
                    raise OSError("fixture write failure")
                write(path, raw)
            with patch.object(connector, "atomic", side_effect=fail_manifest):
                with self.assertRaises(OSError):
                    connector.connect(root, settings, {"command": "new"}, "wsl-unc")
            self.assertEqual(settings.read_bytes(), before)
            self.assertFalse((root / "bridge.json").exists())


if __name__ == "__main__":
    unittest.main()
