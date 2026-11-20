# Releasing

Releases are cut by hand today: build, sign and notarize on a Mac that holds the Developer ID keys, publish the GitHub release, then update Homebrew and the websites. The `release` workflow can do the same with repository secrets (below), but it is not what shipped 0.4.0 and 0.4.1.

## Checklist

1. **Prepare.** Add a `## X.Y.Z` section to `CHANGELOG.md` (it becomes the release notes). Bump the version in `Cargo.toml`, `Cargo.lock`, `macos/Info.plist` (`CFBundleShortVersionString`, and `CFBundleVersion` by one) and the `.TH` line of `man/macora.1`. Update `docs/TESTING.md` if the checks changed.
2. **Test.** The gates in [CONTRIBUTING.md](../CONTRIBUTING.md), then `scripts/verify-cli.py` and `scripts/verify-diagnose.py` against the release binary. Refresh screenshots if the UI changed (`./scripts/build-macos.sh && ./scripts/shots.sh`).
3. **Push `main` and wait for `verify` to pass** on the exact commit you will tag.
4. **Package, sign, notarize** (next section). Do not tag yet.
5. **Verify the downloaded files** (section below).
6. **Publish.** Create the release with the tag pointing at the verified commit. Disable the `release` workflow first and re-enable it afterwards, so a tag push cannot trigger a CI build that uploads unsigned files to the release.
   ```sh
   gh workflow disable release
   gh release create vX.Y.Z release/* --target "$(git rev-parse HEAD)" --title "MACORA X.Y.Z" -F notes.md
   gh workflow enable release
   ```
   Use the full commit SHA; an abbreviated or invented one is rejected.
7. **Homebrew.** `scripts/update-homebrew.sh X.Y.Z <tap checkout>` reads the published `.sha256` files and rewrites `Formula/macora.rb` and `Casks/macora.rb` in `zyvorai/homebrew-tap`. Commit and push the tap, then bump the templates in `packaging/homebrew/`.
8. **Site and docs.** Replace the old version in `site/index.html` and `README.md` (download links, validation line). In `zyvor-web`, `docs/macora.mdx` names the DMG; the product page links to `releases/latest`. Open a PR there.
9. **Wiki.** Update `wiki/` if install steps or commands changed, then `scripts/sync-wiki.sh`.
10. **Check it live.** Pages shows the new version; `brew fetch` works for formula and cask; do a fresh `brew install` of both (below).

## Package, sign and notarize

`scripts/package-mac.sh X.Y.Z` writes the CLI tarball, `Macora-X.Y.Z.dmg`, `Macora-X.Y.Z.pkg` and a `.sha256` for each into `release/`. Without signing variables the output is ad-hoc signed and **not notarized**. With them it signs, submits to Apple's notary service, waits, and staples:

```sh
MACORA_SIGN_IDENTITY="Developer ID Application: <team>" \
DEVELOPER_ID_INSTALLER="Developer ID Installer: <team>" \
NOTARY_PROFILE=<notarytool keychain profile> \
scripts/package-mac.sh X.Y.Z
```

- The CLI tarball is signed with the hardened runtime but **not notarized**; the DMG and pkg are notarized and stapled.
- The app bundle is built for the machine's architecture (Apple silicon); the CLI is universal when both Apple targets are installed.
- `productbuild`/`pkgbuild --sign` needs the keychain unlocked in a **GUI session**. Over plain SSH it fails with `errSecInteractionNotAllowed`. Run the packaging from a `.command` file opened with `open -a Terminal`, which writes a log and a `.done` marker you can poll. The first use of the Installer key shows a keychain prompt; choose Always Allow.
- A Mac that was asleep can come back with a new address and a locked keychain. Confirm its SSH host key against your saved fingerprint before trusting the new address.

### CI signing (optional)

Add repository secrets: `MACOS_CERTS_P12` (base64 .p12 with the Developer ID Application and Installer identities), `MACOS_CERTS_PASSWORD`, `DEVELOPER_ID_APP`, `DEVELOPER_ID_INSTALLER`, and an App Store Connect API key: `NOTARY_KEY_P8` (base64), `NOTARY_KEY_ID`, `NOTARY_ISSUER`.

## Verify the downloaded files

Download the assets back from the release, not from your build folder:

```sh
shasum -a 256 -c *.sha256
xcrun stapler validate Macora-X.Y.Z.dmg Macora-X.Y.Z.pkg
spctl -a -vv -t open --context context:primary-signature Macora-X.Y.Z.dmg   # Notarized Developer ID
spctl -a -vv -t install Macora-X.Y.Z.pkg
hdiutil attach Macora-X.Y.Z.dmg -nobrowse -mountpoint m
spctl -a -vv m/Macora.app; codesign --verify --deep --strict m/Macora.app
defaults read "$PWD/m/Macora.app/Contents/Info" CFBundleShortVersionString
python3 scripts/verify-cli.py m/Macora.app/Contents/Resources/macora
```

Also run the pkg `postinstall` against a scratch root: it must link `/usr/local/bin/macora` and, when the target is unwritable, print a message and still exit 0.

## Fresh Homebrew install

```sh
brew install --formula zyvorai/tap/macora && python3 scripts/verify-cli.py "$(brew --prefix macora)/bin/macora"
brew uninstall --formula zyvorai/tap/macora
brew install --cask zyvorai/tap/macora && open -n /Applications/Macora.app
brew uninstall --cask zyvorai/tap/macora
```

Test the installed path, not `macora` by name: an older copy earlier on `PATH` is what that runs. Do not start the app by running `Contents/MacOS/Macora --help`; it opens the GUI and never returns.

## Repository settings

Topics: `macos` `swiftui` `rust` `network` `ifconfig` `cli` `tui` `diagnostics` `wifi` `dns` `zero-dependency`. Enable private vulnerability reporting and GitHub Pages (source: GitHub Actions). Dependabot updates the GitHub Actions versions; let `verify` pass on those PRs before merging.
