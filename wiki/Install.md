# Install

Requires macOS 14 or newer. Releases are for Apple silicon.

## Download (recommended)

Get `Macora-X.Y.Z.dmg` (drag to Applications) or `Macora-X.Y.Z.pkg` from the [Releases page](https://github.com/zyvorai/zyvor-macora/releases). Both are signed with Zyvor AI Labs' Developer ID, **notarized by Apple** and stapled, so they open without a Gatekeeper warning. Each file has a `.sha256`:

```sh
shasum -a 256 -c Macora-X.Y.Z.dmg.sha256
```

The pkg also links the bundled command line tool to `/usr/local/bin/macora`. If that folder cannot be written the install still succeeds, and you can run it from `/Applications/Macora.app/Contents/Resources/macora`.

## Homebrew

```sh
brew install --cask zyvorai/tap/macora   # the app, and `macora` on your PATH
brew install zyvorai/tap/macora          # the command line tool only
```

Install one or the other: both provide the `macora` command.

Recent Homebrew requires tap trust. If it reports that `zyvorai/tap` is not trusted, run `brew trust --cask zyvorai/tap/macora` (or `brew trust --formula zyvorai/tap/macora` for the CLI) and install again. Warnings about other taps you have installed are unrelated.

## Command line tarball

`macora-X.Y.Z-arm64-apple-darwin.tar.gz` contains the binary, man page and shell completions. It is signed but **not notarized**; if macOS blocks it after download, run `xattr -d com.apple.quarantine macora` once.

## From source

```sh
git clone https://github.com/zyvorai/zyvor-macora.git && cd zyvor-macora
./scripts/install-macos.sh ~/.local     # CLI, man page, completions
./scripts/build-macos.sh                # tests, then dist/Macora.app
```

Needs the Xcode command line tools and Rust. There are no external Rust crates or Swift packages.

## Check which `macora` you are running

```sh
which -a macora
macora version
```

An older copy earlier on `PATH` hides a newer install. See [[Troubleshooting]].

## Uninstall

Drag Macora to the Trash, or `brew uninstall` the cask or formula. It runs no background service and leaves nothing except its preferences.
