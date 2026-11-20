"""Check a release ELF using the same Core checkout as the linked SDK."""

import json
import os
from pathlib import Path
import subprocess
import sys


TARGET_OPTIONS = {
    "x86_64-unknown-linux-gnu": ["--machine", "x86_64"],
    "thumbv7em-none-eabihf": ["--machine", "arm", "--float-abi", "hard"],
    "riscv32imac-unknown-none-elf": ["--machine", "riscv32", "--float-abi", "soft"],
}


def main():
    target = sys.argv[1]
    if target not in TARGET_OPTIONS:
        sys.exit(f"Unsupported Xila target: {target}")

    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--locked", "--format-version", "1",
        "--features", "guest", "--filter-platform", target,
    ]))
    root = metadata["resolve"]["root"]
    node = next(node for node in metadata["resolve"]["nodes"] if node["id"] == root)
    sdk_id = next(dep["pkg"] for dep in node["deps"] if dep["name"] == "xila_sys")
    packages = {package["id"]: package for package in metadata["packages"]}
    sdk_dir = Path(packages[sdk_id]["manifest_path"]).parent
    core = Path(subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], cwd=sdk_dir, text=True,
    ).strip())

    binary = next(item["name"] for item in packages[root]["targets"] if "bin" in item["kind"])
    image = Path(metadata["target_directory"]) / target / "release" / binary
    if not image.is_file():
        sys.exit(f"ELF missing: {image}\nRun mise run build for {target} first.")

    checker = core / "tools/sdk/check_elf.py"
    os.execv(sys.executable, [sys.executable, str(checker), str(image), *TARGET_OPTIONS[target]])


if __name__ == "__main__":
    main()
