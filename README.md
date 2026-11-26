<h1>
<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/brand/header-dark.svg">
  <img src=".github/brand/header-light.svg" alt="Xila Native Rust app template — A Rust starter for your first native Xila application." width="1280">
</picture>
</h1>

[Documentation](https://www.xila.dev/docs/) ·
[Community](https://matrix.to/#/#xila:anneraud.fr)

Build a small `no_std` Rust application for the Xila embedded OS. The starter
prints a greeting and its arguments to stdout. Pass `--fail` as the first
argument to print a diagnostic to stderr and return status 1. A normal run
returns 0, and a stream write failure returns 2.

The application body in `src/lib.rs` backs two native modes: the `guest`
feature builds a loadable static-PIE ELF, and the `linked` feature builds a
library whose entry mounts directly into Xila firmware. See
[Two native modes](#two-native-modes).

## Requirements

Use an x86-64 Linux host with Git, [Rustup](https://rustup.rs/),
[mise](https://mise.jdx.dev/) and Python 3 available to uv. Rustup selects the
pinned nightly and installs `rust-src`, the target libraries and the lint tools
from `rust-toolchain.toml`. The first build needs network access to fetch the
toolchain, the pinned Core revision and Cargo dependencies. The guest build uses
Rustup's bundled `rust-lld`.

## Create your application

1. Select **Use this template** on GitHub and clone your new repository, or
   clone this repository directly.
2. Set the package name in `Cargo.toml` and the matching `hello::` crate path in
   `src/main.rs`, then run `cargo generate-lockfile`.
3. Edit `src/lib.rs` to implement your application: its `run` backs both native
   modes. `src/main.rs` only carries the guest entry shim, allocator and panic
   handler.
4. Replace the README header, `.github/brand/` assets and README footer with
   your project's own material. Update the copyright holder in `LICENSE` for
   your own work and retain the template's MIT notice as required.

## Build and check

Review the mise configuration before trusting it.

```sh
mise trust
mise run build
mise run check
```

The default output is `target/x86_64-unknown-linux-gnu/release/hello`.
The filename follows the Cargo binary name. Set `XILA_SDK_TARGET` for both
tasks to select another target.

```sh
XILA_SDK_TARGET=thumbv7em-none-eabihf mise run build
XILA_SDK_TARGET=thumbv7em-none-eabihf mise run check
```

| Target | Xila platform |
| --- | --- |
| `x86_64-unknown-linux-gnu` | Linux x86-64 host loader |
| `thumbv7em-none-eabihf` | Arm Cortex-M4F, hard float |
| `riscv32imac-unknown-none-elf` | RV32IMAC, soft float |

`cargo xila-build` and `cargo xila-clippy` enable the `guest` feature and apply
`.cargo/guest.toml`. Cargo resolves that config path relative to the current
directory, so run the aliases from the repository root (from a subdirectory they
fail to read the file). Always pass `--target` with one of the targets above:
without it, the guest linker options also apply to host build scripts, and the
build fails at link time.

```sh
cargo xila-build --locked --release --target thumbv7em-none-eabihf
cargo xila-clippy --locked --release --target thumbv7em-none-eabihf -- -D warnings
```

Cargo's nightly `build-std` rebuilds `core`, `alloc` and `compiler_builtins`
as position-independent code. Nightly also lets `xila-sys/link` forward the
linker script and target arguments to the application. Keep the host test build
separate with `cargo test --locked` and `cargo clippy --locked --all-targets`,
which use the normal host sysroot without the guest feature. The ELF checker
finds Core through locked Cargo metadata, so it checks against the same ABI
revision used for the build. That needs `xila-sys` from a Git checkout of Core
(a `git` dependency, or a `[patch]` entry pointing at a Core clone); a path or
vendored copy has no checker next to it and `mise run check` reports that. The
checker also fails when the `rust-toolchain.toml` channel differs from the
`nightly_version` of Core at the pinned revision.

## Two native modes

| Mode | Feature | Artifact | Entry |
| --- | --- | --- | --- |
| Loadable ELF | `guest` (set by `cargo xila-build`) | static-PIE ELF for the native loader | `_xila_start` in `src/main.rs` |
| Statically linked | `linked` | library linked into the firmware | `hello::xila_entry` in `src/lib.rs` |

The features are mutually exclusive: `xila-sys` rejects enabling its `link` and
`linked` features together, so keep the `features = ["link"]` off the
dependency line and let each mode turn its own `xila-sys` feature on.

With `linked`, build the library with plain cargo on stable Rust: no nightly
`build-std`, no guest linker config, no ELF note and no `_xila_start`, and no
allocator or panic handler of its own (the firmware's apply).

```sh
mise run check:linked
# cargo check --locked --lib --features linked
# cargo clippy --locked --lib --features linked -- -D warnings
```

In a Xila firmware, mount the entry with the `xila` facade and then like any
other executable with `mount_executables!`. The host hands each running program
its own startup block, so several linked applications can run at once.

```rust
xila::executable::linked_executable!(unsafe Hello => hello::xila_entry, stack_size = 32 * 1024);
```

## Editor setup

`mod guest` only exists with the `guest` feature, so rust-analyzer shows it as
inactive by default. The repository ships `.vscode/settings.json` with
`"rust-analyzer.cargo.features": ["guest"]`. In another editor, set the same
option (`rust-analyzer.cargo.features = ["guest"]`), or `["linked"]` to inspect
`xila_entry` in `src/lib.rs`. The host sysroot is enough
for analysis: `cargo clippy --locked --all-targets --features guest` works
without the guest sysroot. Real guest builds still go through `cargo xila-build`.

## Run through Core's host loader

No released Xila build runs native executables yet. Platform support is tracked
in [xila-project/platforms#115](https://github.com/xila-project/platforms/issues/115).
Today the x86-64 ELF runs through Core's host loader tests, which install the ELF
in a test filesystem and launch it with the Xila ABI.

In a separate Core checkout at the revision pinned in `Cargo.toml`, add this
test to `libraries/executable/tests/sdk.rs`.
It uses the SDK test's existing `setup`, `install` and `launch` helpers.

```rust
#[task::test]
async fn runs_native_rust_template() {
    let (context, parent) = setup().await;
    let image = std::fs::read(std::env::var_os("XILA_TEMPLATE_ELF").unwrap()).unwrap();
    let path = "/hello";
    install(context.virtual_file_system, parent, path, &image).await;

    let (status, stdout, stderr) = launch(context, parent, path, "hello", &["world"]).await;
    assert_eq!(status, 0);
    assert_eq!(stdout, b"Hello from Rust on Xila!\narg: world\n");
    assert!(stderr.is_empty());
}
```

From that Core checkout, set the ELF path and run the focused test with Core's
host prerequisites. Use Core's `ci:packages-host` mise task on Debian or Ubuntu,
and install the Rust version from Core's `mise.toml` with Rustup.

```sh
XILA_TEMPLATE_ELF=/absolute/path/to/this/template/target/x86_64-unknown-linux-gnu/release/hello \
  cargo +1.97.0 test --locked -p xila-executable --features native-dev --test sdk -- runs_native_rust_template --exact
```

## Update the SDK pin

Review a revision on Core's `main` and put its full SHA in the `xila-sys` `rev`
in `Cargo.toml`. Set `channel` in `rust-toolchain.toml` to the `nightly_version`
from that same revision's `mise.toml`.

Regenerate `Cargo.lock`, then build and check all targets before committing the
pin. Fix any reported failure before proceeding.

```sh
cargo generate-lockfile
for target in x86_64-unknown-linux-gnu thumbv7em-none-eabihf riscv32imac-unknown-none-elf; do
  XILA_SDK_TARGET="$target" mise run build &&
    XILA_SDK_TARGET="$target" mise run check || break
done
```

## Nanobom

To publish vulnerability reports, create a Nanobom project named after the
lowercased GitHub repository. Set the Actions secret `NANOBOM_TOKEN` to its
upload token and the Actions variable `NANOBOM_ENABLED` to `true`. For your own
server or organization, set the Actions variables `NANOBOM_URL` and
`NANOBOM_ORGANIZATION` to match the project. You can add the project's public
badge to this README.

## License

This template is released under the [MIT License](LICENSE), except for the
`.github/brand/` header assets, which are covered by [their own notice](.github/brand/README.md).

<!-- xila:footer:start -->

---

Part of **[Xila](https://www.xila.dev/)** ·
[Repositories](https://github.com/xila-project) ·
[Community](https://matrix.to/#/#xila:anneraud.fr)

Special thanks to [Martin Legand](https://www.behance.net/martinlegand) for generously designing the Xila logo.

<!-- xila:footer:end -->
