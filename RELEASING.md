# Releasing Portier

Cutting a release is tag-driven. `.github/workflows/release.yml` builds binaries
for macOS (arm64 + x64), Linux (x64), and Windows (x64), then publishes a GitHub
Release with `CHANGELOG.md` as the body.

## 1. Prepare

- [ ] Bump `version` in the workspace `Cargo.toml` (and the package manifests
      below that pin it: `npm-installer/package.json`, `homebrew-portier/Formula/portier.rb`,
      `scoop/portier.json`).
- [ ] Update `CHANGELOG.md` with the new version + date.
- [ ] `cargo test --workspace` and `cargo clippy -- -D warnings` are green.

## 2. Tag

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow runs and attaches these assets to the GitHub Release:

```
portier-aarch64-apple-darwin.tar.gz
portier-x86_64-apple-darwin.tar.gz
portier-x86_64-unknown-linux-gnu.tar.gz
portier-x86_64-pc-windows-msvc.zip
```

## 3. Fill in the SHA256 placeholders

Each package manifest ships with zeroed hashes until the assets exist. After the
release is published:

```bash
# Compute hashes for each asset
shasum -a 256 portier-*.tar.gz portier-*.zip
```

- [ ] **Homebrew** — paste the macOS/Linux `.tar.gz` hashes into
      `homebrew-portier/Formula/portier.rb`, then push to the tap repo
      (`portier-cli/homebrew-tap`). Users: `brew install portier-cli/tap/portier`.
- [ ] **Scoop** — paste the Windows `.zip` hash into `scoop/portier.json` and
      push to the bucket repo. Users: `scoop install portier`.

## 4. npm

```bash
cd npm-installer
npm publish --access public   # @portier/cli
```

The package downloads the matching binary on `postinstall` and exposes it via the
`bin/portier.js` launcher.

## 5. Winget

Generate and submit manifests at release time (no hand-maintained files needed):

```bash
wingetcreate update Portier.Portier \
  --version 0.1.0 \
  --urls https://github.com/vaibhavtomar/portier/releases/download/v0.1.0/portier-x86_64-pc-windows-msvc.zip \
  --submit
```

## 6. Verify

- [ ] `brew install …` / `scoop install …` / `npm i -g @portier/cli` each land a
      working `portier --version`.
