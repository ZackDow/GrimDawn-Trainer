"""Build the Windows x64 session bridge used on Windows and inside Wine."""
import os
from pathlib import Path
import shutil
import subprocess
ROOT=Path(__file__).resolve().parent.parent
cc=os.environ.get('GRIM_MINGW_CC') or shutil.which('x86_64-w64-mingw32-gcc-posix') or shutil.which('x86_64-w64-mingw32-gcc')
if not cc: raise SystemExit('Install a MinGW-w64 x64 C compiler or set GRIM_MINGW_CC.')
out=ROOT/'runtime/bin';out.mkdir(exist_ok=True)
common=[cc,'-std=c11','-O2','-Wall','-Wextra','-static-libgcc','-Wno-cast-function-type','-Wno-misleading-indentation']
subprocess.run(common+['-municode','runtime/bridge.c','-o',str(out/'grim-bridge.exe')],cwd=ROOT,check=True)
sources=['runtime/session.c','runtime/vendor/minhook/src/hook.c','runtime/vendor/minhook/src/buffer.c','runtime/vendor/minhook/src/trampoline.c','runtime/vendor/minhook/src/hde/hde64.c']
subprocess.run(common+['-shared']+sources+['-lbcrypt','-o',str(out/'grim-session-v8.dll')],cwd=ROOT,check=True)
print(out)
