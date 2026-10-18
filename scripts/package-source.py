"""Package source and compiled trainer runtime; never bundle game/save data."""
from pathlib import Path
import json
import zipfile

ROOT = Path(__file__).resolve().parent.parent
VERSION = json.loads((ROOT / "package.json").read_text())["version"]
OUTPUT = ROOT / "release" / f"grim-dawn-trainer-{VERSION}-source.zip"
ROOT_FILES = [".gitignore", "Cargo.toml", "Cargo.lock", "package.json", "package-lock.json", "tsconfig.json", "vite.config.ts", "index.html", "README.md", "LICENSE"]
TREES = ["src", "scripts", "assets", "docs", "crates", ".github", "runtime"]
NATIVE = ["src-tauri/Cargo.toml", "src-tauri/build.rs", "src-tauri/tauri.conf.json"]
ICONS = ["32x32.png", "64x64.png", "128x128.png", "128x128@2x.png", "icon.png", "icon.ico", "icon.icns"]

def package_source():
    files = [ROOT / name for name in ROOT_FILES + NATIVE]
    for tree in TREES + ["src-tauri/src", "src-tauri/capabilities"]:
        files.extend(p for p in (ROOT / tree).rglob("*") if p.is_file() and "__pycache__" not in p.parts and p.name not in {"grim-session-v2.dll", "grim-session-v3.dll", "grim-session-v4.dll", "grim-session-v5.dll", "grim-session-v6.dll", "grim-session-v7.dll"})
    files.extend(ROOT / "src-tauri/icons" / name for name in ICONS)
    OUTPUT.parent.mkdir(exist_ok=True)
    with zipfile.ZipFile(OUTPUT, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(set(files)):
            assert path.is_file() and not path.is_symlink(), path
            assert path.suffix.lower() not in {".gdc", ".arz", ".dbr", ".bin"}, path
            archive.write(path, "GrimDawn Trainer/" + str(path.relative_to(ROOT)))
    with zipfile.ZipFile(OUTPUT) as archive:
        assert archive.testzip() is None
    print(OUTPUT)

if __name__ == "__main__":
    package_source()
