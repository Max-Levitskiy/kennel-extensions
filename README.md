# kennel-extensions

This is the default extension index for [kennel](https://github.com/Max-Levitskiy/kennel). `index.toml` lists the extensions kennel's GUI offers out of the box, and `GuiConfig::default()` points at this repo's `index.toml` on the `main` branch.

To publish an extension, push a tag of the form `<extension-name>-vX.Y.Z` (e.g. `sd-keepalive-v0.1.0`). The `release-extension` GitHub Actions workflow builds the corresponding crate to `wasm32-unknown-unknown` and attaches the `.wasm` asset to a GitHub Release. Updating `index.toml` with the resulting release URL and sha256 is currently a manual follow-up step.
