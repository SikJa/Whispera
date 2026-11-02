#!/usr/bin/env python3
"""Connect WSL Claude Code to the installed Whispera readaloud.4 collector.

No dependencies, network calls, authentication files or Claude prompts. Run in
the WSL distribution where Claude Code runs, after connecting once in Whispera.
The existing app's Disconnect button can undo this connection via the UNC path.
"""
import argparse
import json
import os
from pathlib import Path
import shlex
import stat
import subprocess
import sys
import tempfile
import uuid

MAX_FILE = 262144


def command(args):
    return subprocess.check_output(args, text=True, timeout=15).strip()


def linux_path(windows):
    if windows.startswith("\\\\?\\UNC\\"):
        windows = "\\\\" + windows[8:]
    elif windows.startswith("\\\\?\\"):
        windows = windows[4:]
    return Path(command(["wslpath", "-u", windows])).resolve()


def read(path):
    raw = path.read_bytes() if path.exists() else None
    if raw is not None and len(raw) > MAX_FILE:
        raise ValueError("Archivo de configuración demasiado grande.")
    value = json.loads(raw.decode("utf-8-sig")) if raw is not None else {}
    if not isinstance(value, dict):
        raise ValueError("La configuración debe ser un objeto JSON.")
    return raw, value


def encode(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def atomic(path, raw):
    path.parent.mkdir(parents=True, exist_ok=True)
    mode = stat.S_IMODE(path.stat().st_mode) if path.exists() else 0o600
    fd, temporary = tempfile.mkstemp(prefix=".whispera-", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as stream:
            os.fchmod(stream.fileno(), mode)
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        Path(temporary).unlink(missing_ok=True)


def backup(path, raw):
    if raw is None:
        return
    target = path.with_name(path.stem + ".whispera-backup-" + uuid.uuid4().hex + ".json")
    with open(os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb") as stream:
        stream.write(raw)


def connect(directory, settings, installed, windows_settings):
    """Preserve unrelated fields and roll back partial writes without clobbering edits."""
    manifest = directory / "bridge.json"
    before_manifest, old = read(manifest)
    before_settings, current = read(settings)
    if before_manifest is not None and old.get("installed") == installed and old.get("settings") == windows_settings and current.get("statusLine") == installed:
        return False

    changes = []
    if before_manifest is not None:
        old_settings = linux_path(old["settings"])
        old_raw, old_config = read(old_settings)
        if old_config.get("statusLine") != old.get("installed"):
            raise ValueError("La barra anterior cambió fuera de Whispera. No se modificó.")
        if old_settings == settings:
            previous = old.get("previous")
        else:
            previous = current.get("statusLine")
            if old.get("previous") is None:
                old_config.pop("statusLine", None)
            else:
                old_config["statusLine"] = old["previous"]
            changes.append((old_settings, old_raw, encode(old_config)))
    else:
        previous = current.get("statusLine")

    current["statusLine"] = installed
    changes.append((settings, before_settings, encode(current)))
    changes.append((manifest, before_manifest, encode({
        "settings": windows_settings, "previous": previous, "installed": installed,
    })))
    for path, before, _ in changes:
        backup(path, before)

    # Remove a previous installation/account's snapshot before the new hook can run.
    (directory / "claude.json").unlink(missing_ok=True)
    written = []
    try:
        for path, before, after in changes:
            if (path.read_bytes() if path.exists() else None) != before:
                raise ValueError("La configuración cambió durante la conexión. Volvé a intentar.")
            atomic(path, after)
            written.append((path, before, after))
    except Exception:
        for path, before, after in reversed(written):
            if path.read_bytes() == after:
                if before is None:
                    path.unlink()
                else:
                    atomic(path, before)
        raise
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app-data", type=Path, help="Carpeta ai-usage de Whispera, como ruta /mnt/c/…")
    args = parser.parse_args()
    if not os.environ.get("WSL_DISTRO_NAME"):
        raise ValueError("Ejecutá este conector dentro de WSL.")
    directory = args.app_data
    if directory is None:
        appdata = command(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", "[Console]::Write($env:APPDATA)"])
        directory = linux_path(appdata) / "app.whispera.desktop.preview" / "ai-usage"
    directory = directory.resolve()
    collector = directory / "WhisperaClaudeUsage.exe"
    if not collector.is_file():
        raise ValueError("Abrí Whispera → Uso de IA → Conectar Claude Code una vez para preparar el conector.")
    config = Path(os.environ.get("CLAUDE_CONFIG_DIR", str(Path.home() / ".claude")))
    if not config.is_absolute():
        raise ValueError("CLAUDE_CONFIG_DIR debe ser una ruta absoluta.")
    settings = (config / "settings.json").resolve()
    output = command(["wslpath", "-w", str(directory / "claude.json")])
    installed = {"type": "command", "command": shlex.join([str(collector), output])}
    # Validate WSL interoperability and sanitization with a temporary fixture only.
    with tempfile.TemporaryDirectory(prefix=".wsl-check-", dir=directory) as temporary:
        target = Path(temporary) / "fixture.json"
        fixture = {"rate_limits": {"five_hour": {"used_percentage": 23}}, "transcript_path": "discard"}
        subprocess.run([str(collector), command(["wslpath", "-w", str(target)])],
                       input=json.dumps(fixture), text=True, stdout=subprocess.DEVNULL,
                       stderr=subprocess.DEVNULL, check=True, timeout=15)
        _, clean = read(target)
        if set(clean) != {"schema", "observedAt", "rate_limits"} or clean["rate_limits"]["five_hour"]["used_percentage"] != 23:
            raise ValueError("No se pudo validar el conector instalado.")
    changed = connect(directory, settings, installed, command(["wslpath", "-w", str(settings)]))
    print("Conexión WSL preparada." if changed else "La conexión WSL ya estaba preparada.")
    print("Configuración: " + str(settings))
    print("Usá Claude normalmente. Tras su próxima respuesta, abrí Uso de IA → Actualizar cuotas.")
    print("Para deshacerlo: Desconectar Claude Code en Whispera. Se conservaron copias de seguridad junto a las configuraciones.")


if __name__ == "__main__":
    try:
        main()
    except ValueError as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
    except Exception:
        print("No se completó la conexión. Revisá que WSL pueda ejecutar programas de Windows y que Whispera ya tenga preparado el conector. Revisá también los permisos de los archivos de configuración; las copias se guardan junto a esos archivos.", file=sys.stderr)
        sys.exit(1)
