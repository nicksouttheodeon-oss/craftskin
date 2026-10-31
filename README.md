# CraftSkin 0.5.3 — FilmCraft v0.15.0 Support Implementation

CraftSkin is a **Rust source injector** for a separate FilmCraft checkout. It does not edit the official ArtCraft Launcher installation.

## Build injector

```sh
cargo test
cargo run --release
```

## Upgrade from 0.4/0.5

1. Back up your FilmCraft checkout. Stop FilmCraft and restore the earlier integration using the version of CraftSkin you used to install it. Verify `git status --short` before re-injecting.
2. Open CraftSkin 0.5.2 and point it at a **clean separate FilmCraft Rust source checkout**, not the launcher installation.
3. Select **Inject CraftSkin into source**. A mismatched source fragment produces a precise error and the installer does not write partial changes.
4. Build and launch the modified editor:

```sh
cd ~/src/filmcraft-craftskin
cargo build --release -p filmcraft
FILMCRAFT_DATA_DIR="$HOME/.local/share/filmcraft-craftskin" ./target/release/filmcraft --empty
```

5. In the custom build, open **Edit → Preferences → Appearance** and scroll to the **CraftSkin** section. Selecting a preset immediately saves `~/.config/craftskin/filmcraft.json` (unless `CRAFTSKIN_CONFIG` overrides the location).

## Compatibility and safety

The injector plans all edits before writing, backs up modified files, and rejects unfamiliar or ambiguous patches. It cannot guarantee compatibility with every FilmCraft commit. For full source-level theme functionality a successful injection **and** FilmCraft compilation are needed. Existing profiles should be preserved when upgrading. The compiled official FilmCraft in ArtCraft Launcher is unaffected.

If the installer reports a mismatch, copy the **entire** message including `Failed source file:` and the FilmCraft Git commit (`git rev-parse --short HEAD`). Avoid forcing source changes.

## Verification

This package includes a regression test for the generated bridge template and the existing adapter tests. A full Cargo test run and FilmCraft compile against your precise revision are not verified here.
