"""Check a release ELF using the same Core checkout as the linked SDK."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib


SDK_PACKAGE = "xila-sys"
CHECKER = "tools/sdk/check_elf.py"

TARGET_OPTIONS = {
    "x86_64-unknown-linux-gnu": ["--machine", "x86_64"],
    "thumbv7em-none-eabihf": ["--machine", "arm", "--float-abi", "hard"],
    "riscv32imac-unknown-none-elf": ["--machine", "riscv32", "--float-abi", "soft"],
}


def find_core(sdk_dir):
    """Return the Core checkout that contains the SDK, or exit with an explanation."""
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        cwd=sdk_dir, text=True, capture_output=True,
    )
    core = Path(result.stdout.strip()) if result.returncode == 0 else None
    if core is None or not (core / CHECKER).is_file():
        sys.exit(
            f"Cannot find Core's {CHECKER} next to {SDK_PACKAGE} in {sdk_dir}.\n"
            f"This check needs {SDK_PACKAGE} from a Git checkout of Core, which is what a "
            "`git` dependency or a `[patch]` entry that points at a Core clone provides. "
            "A path or vendored copy does not carry the checker: run it from a Core "
            "checkout at the pinned revision instead."
        )
    return core


def check_toolchain(core):
    """The application must build with the nightly that Core builds its SDK with."""
    expected = tomllib.loads((core / "mise.toml").read_text())["vars"]["nightly_version"]
    channel = tomllib.loads(Path("rust-toolchain.toml").read_text())["toolchain"]["channel"]
    if channel != expected:
        sys.exit(
            f"rust-toolchain.toml uses {channel}, but Core at the pinned {SDK_PACKAGE} "
            f"revision uses {expected}. Set `channel` to {expected} and rebuild."
        )


def main():
    target = sys.argv[1]
    if target not in TARGET_OPTIONS:
        sys.exit(f"Unsupported Xila target: {target}")

    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--locked", "--format-version", "1",
        "--features", "guest", "--filter-platform", target,
    ]))
    packages = {package["id"]: package for package in metadata["packages"]}
    root = metadata["resolve"]["root"]
    node = next(node for node in metadata["resolve"]["nodes"] if node["id"] == root)
    # Match by package name: the dependency key may be renamed in Cargo.toml.
    sdk_id = next(
        (dep["pkg"] for dep in node["deps"] if packages[dep["pkg"]]["name"] == SDK_PACKAGE),
        None,
    )
    if sdk_id is None:
        sys.exit(f"{packages[root]['name']} does not depend on {SDK_PACKAGE} with the guest feature.")
    sdk_dir = Path(packages[sdk_id]["manifest_path"]).parent
    core = find_core(sdk_dir)
    check_toolchain(core)

    binary = next(item["name"] for item in packages[root]["targets"] if "bin" in item["kind"])
    image = Path(metadata["target_directory"]) / target / "release" / binary
    if not image.is_file():
        sys.exit(f"ELF missing: {image}\nRun mise run build for {target} first.")

    os.execv(sys.executable, [
        sys.executable, str(core / CHECKER), str(image), *TARGET_OPTIONS[target],
    ])


if __name__ == "__main__":
    main()
