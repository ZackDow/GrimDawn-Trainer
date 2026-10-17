from pathlib import Path
import os,shutil,subprocess
ROOT=Path(__file__).resolve().parent.parent
cc=os.environ.get('GRIM_MINGW_CC') or shutil.which('x86_64-w64-mingw32-gcc-posix') or shutil.which('x86_64-w64-mingw32-gcc')
if not cc: raise SystemExit('Install a MinGW-w64 compiler or set GRIM_MINGW_CC.')
out=ROOT/'.local-test/session-test.exe'
out.parent.mkdir(exist_ok=True)
sources=['runtime/tests/session_test.c','runtime/vendor/minhook/src/hook.c','runtime/vendor/minhook/src/buffer.c','runtime/vendor/minhook/src/trampoline.c','runtime/vendor/minhook/src/hde/hde64.c']
subprocess.run([str(cc),'-std=c11','-O2','-static-libgcc']+sources+['-lbcrypt','-o',str(out)],cwd=ROOT,check=True)
print(out)
