"""Find an installed Python scripting plugin without importing native Enduro/X."""
import os
from pathlib import Path
import re
import shutil
import subprocess

probe = """
import importlib.util
from importlib.machinery import PathFinder
from pathlib import Path
package = importlib.util.find_spec('endurox')
if package and package.submodule_search_locations:
    extension = PathFinder.find_spec('endurox._endurox', package.submodule_search_locations)
    if extension and extension.origin and Path(extension.origin).is_file():
        print(extension.origin)
"""

# PATH can contain several Python versions while `python3` still names an older
# system interpreter. Probe the default first, then available versioned commands.
versions = set()
for directory in os.get_exec_path():
    try:
        for name in os.listdir(directory):
            if re.fullmatch(r"python3\.\d+", name):
                versions.add(name)
    except OSError:
        pass
interpreters = [os.environ.get("PYTHON", "python3")]
interpreters += sorted(versions, key=lambda name: int(name.split(".")[1]), reverse=True)
seen = set()
for interpreter in interpreters:
    executable = shutil.which(interpreter)
    if not executable or os.path.realpath(executable) in seen:
        continue
    seen.add(os.path.realpath(executable))
    try:
        result = subprocess.run([executable, "-c", probe], capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        continue
    if result.returncode == 0 and result.stdout.strip():
        print(result.stdout.strip())
        break
