"""Package the built Linux desktop with its required Windows runtime resources."""
from pathlib import Path
import hashlib
import shutil
import zipfile

ROOT = Path(__file__).resolve().parent.parent
RELEASE = ROOT / "release"
PORTABLE = RELEASE / "grim-dawn-trainer-0.3.0-linux-x86_64"
PORTABLE.mkdir(parents=True, exist_ok=True)
(PORTABLE / "runtime").mkdir(exist_ok=True)
files = {
    "target/release/grim-dawn-trainer": "grim-dawn-trainer",
    "scripts/start.sh": "start.sh",
    "runtime/bin/grim-bridge.exe": "runtime/grim-bridge.exe",
    "runtime/bin/grim-session-v8.dll": "runtime/grim-session-v8.dll",
    "runtime/vendor/minhook/LICENSE.txt": "runtime/MinHook-LICENSE.txt",
    "README.md": "README.md",
    "LICENSE": "LICENSE",
    "docs/VALIDATION.md": "docs/VALIDATION.md",
}
for source, destination in files.items():
    target = PORTABLE / destination
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / source, target)
archive_path = RELEASE / (PORTABLE.name + ".zip")
with zipfile.ZipFile(archive_path, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
    for destination in sorted(files.values()):
        archive.write(PORTABLE / destination, PORTABLE.name + "/" + destination)
with zipfile.ZipFile(archive_path) as archive:
    assert archive.testzip() is None
deb = RELEASE / "grim-dawn-trainer_0.3.0_amd64.deb"
shutil.copy2(ROOT / "target/release/bundle/deb/Grim Dawn Trainer_0.3.0_amd64.deb", deb)
artifacts = [archive_path, deb, RELEASE / "grim-dawn-trainer-0.3.0-source.zip"]
(RELEASE / "SHA256SUMS-0.3.0").write_text("".join(
    hashlib.sha256(path.read_bytes()).hexdigest() + "  " + path.name + "\n"
    for path in artifacts
))
for path in artifacts:
    print(f"{path.name}: {path.stat().st_size:,} bytes")
