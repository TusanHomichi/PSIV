"""Python gate adapter for the Rust-owned source guard; no scanning policy here."""
from functools import lru_cache
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CRATES = ("psiv-core", "psiv-data", "psiv-runtime")


@lru_cache(maxsize=1)
def binary():
    supplied = os.environ.get("PSIV_FLOAT_GUARD_BINARY")
    if supplied:
        return supplied
    # Nix supplies an immutable binary. Local runs build once per worker, under
    # the repository build reservation; gate's exclusive holder nests safely.
    subprocess.run([sys.executable, str(ROOT / 'tools/heavy_lock.py'), 'run',
                    '--class', 'build', '--', 'cargo', 'build', '--quiet',
                    '--manifest-path', str(ROOT / 'rust/Cargo.toml'),
                    '-p', 'psiv-float-guard'], check=True)
    metadata = subprocess.run(['cargo', 'metadata', '--no-deps', '--format-version', '1',
                              '--manifest-path', str(ROOT / 'rust/Cargo.toml')],
                             check=True, text=True, capture_output=True)
    suffix = '.exe' if os.name == 'nt' else ''
    return str(Path(json.loads(metadata.stdout)['target_directory']) / 'debug' / ('psiv-float-guard' + suffix))


def request(operation, **fields):
    result = subprocess.run([binary()], input=json.dumps(dict(operation=operation, **fields)),
                            text=True, capture_output=True, check=False)
    if result.returncode:
        raise RuntimeError(result.stderr)
    return json.loads(result.stdout)


def float_sites(source):
    return [tuple(site) for site in request('float', source=source)]


def violations(root):
    return [f"{site['path']}:{site['line']}: float syntax {site['token']}"
            for site in request('scan', root=str(root))]


def doc_examples(source):
    for example in request('docs', source=source):
        yield example['source'], example['lines']
