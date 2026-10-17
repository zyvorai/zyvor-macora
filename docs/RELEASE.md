# Releasing

1. Update `CHANGELOG.md` (a `## X.Y.Z` heading becomes the release notes) and bump `version` in `Cargo.toml` and `macos/Info.plist`.
2. `cargo test --locked && swift test --package-path macos && scripts/smoke-cli.sh`.
3. Refresh screenshots if the UI changed: `./scripts/build-macos.sh && ./scripts/shots.sh`.
4. `git tag vX.Y.Z && git push --tags`. The `release` workflow builds and uploads `macora-X.Y.Z-*.tar.gz`, `Macora-X.Y.Z.dmg`, `Macora-X.Y.Z.pkg` and a `.sha256` for each.

Local dry run: `scripts/package-mac.sh X.Y.Z` writes to `release/`.

## Signing and notarization

Without secrets the artifacts are ad-hoc signed and **not notarized**; say so in the release notes. To sign and notarize, add repository secrets: `MACOS_CERTS_P12` (base64 .p12 with the Developer ID Application and Installer identities), `MACOS_CERTS_PASSWORD`, `DEVELOPER_ID_APP`, `DEVELOPER_ID_INSTALLER`, and an App Store Connect API key: `NOTARY_KEY_P8` (base64), `NOTARY_KEY_ID`, `NOTARY_ISSUER`.

The app bundle is built for the runner's architecture (Apple silicon on `macos-latest`); the CLI tarball is universal when both Apple targets are installed.

## Repository settings

Topics: `macos` `swiftui` `rust` `network` `ifconfig` `cli` `tui` `diagnostics` `wifi` `dns` `zero-dependency`. Enable private vulnerability reporting and GitHub Pages (source: GitHub Actions).
