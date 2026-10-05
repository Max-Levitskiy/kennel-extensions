# kennel-extensions

Extensions for [kennel](https://github.com/Max-Levitskiy/kennel), and the store
index its GUI reads by default:
`https://raw.githubusercontent.com/Max-Levitskiy/kennel-extensions/main/index.toml`.

| Extension | What it watches |
|---|---|
| `sd-keepalive` | touches a file on the SD card so its reader's PCIe link never idle-parks |
| `gdrive-watchdog` | Google Drive FileProvider stalls; restarts Drive |
| `sketchybar-watchdog` | sketchybar missing from any display; restarts it (needs `kennel-barprobe` from Kennel.app) |

Each extension is a `cdylib` crate built to `wasm32-unknown-unknown` against
`kennel-guest-sdk`, plus a `manifest.toml` that declares the host capabilities it
may use.

## Releasing

Bump `version` in both the crate's `Cargo.toml` and its `manifest.toml`, commit,
then push a tag `<name>-v<version>`:

```bash
git tag sketchybar-watchdog-v0.2.0 && git push origin sketchybar-watchdog-v0.2.0
```

The `release-extension` workflow checks that the tag, crate and manifest agree,
runs the crate's tests, builds it, publishes `monitor.wasm` and `manifest.toml` as
a GitHub Release, and rewrites that extension's entry in `index.toml` on `main`
with the release URLs and both sha256 hashes. Don't edit `index.toml` by hand.

## Developing

```bash
rustup target add wasm32-unknown-unknown
cargo test --workspace
cargo build --release --target wasm32-unknown-unknown --workspace
```

`kennel-guest-sdk` comes from kennel's `main` branch, pinned by `Cargo.lock`.
After an SDK change there, run `cargo update -p kennel-guest-sdk`.
