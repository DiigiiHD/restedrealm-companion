# Releasing RestedRealm Companion

How a new version reaches players, and the one-time setup that makes it possible.

## How updates reach players

Pushing code to Git does not change anyone's app. A version goes out only when a release is **published**:

1. A version tag such as `v0.2.1` is pushed. It must match `version` in `app/Cargo.toml`.
2. The `Release RestedRealm Companion` workflow builds the installer on Windows, signs the update with the updater key, and makes a **draft** GitHub Release with the installer, its `.sig` file and `latest.json`.
3. The owner checks the draft and presses **Publish release**.
4. Every installed app looks at `releases/latest/download/latest.json` about a minute after it starts and then every hour. When it finds a newer version with a valid signature, it installs it quietly (a per-user install needs no permission prompt) and starts again. The addon in World of Warcraft is updated from the new app the next time the game is closed.

A draft is invisible to the apps, so nothing reaches players until it is published. An update signed with any other key is refused.

## One-time setup (owner)

Done once, before the first release. It needs the owner's GitHub account, which is why a coding assistant cannot do it.

### 1. Create the updater signing key

Any computer with Node.js works, for example the VPS:

```bash
npx --yes @tauri-apps/cli@2 signer generate -w ~/restedrealm-companion-updater.key
```

It asks for a password. This makes two files:

- `~/restedrealm-companion-updater.key`: the **private** key. Keep it secret.
- `~/restedrealm-companion-updater.key.pub`: the **public** key.

**Back up the private key and its password somewhere safe outside the VPS**, for example a password manager. If it is lost, installed apps can no longer be updated and every player would have to download a new version by hand once.

### 2. Put the keys into GitHub

In `DiigiiHD/restedrealm-companion` on GitHub: **Settings > Secrets and variables > Actions**.

On the **Secrets** tab, choose **New repository secret** twice:

| Name | Value |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | the whole contents of the `.key` file (`cat ~/restedrealm-companion-updater.key`) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | the password from step 1 |

On the **Variables** tab, choose **New repository variable**:

| Name | Value |
| --- | --- |
| `TAURI_UPDATER_PUBKEY` | the whole contents of the `.key.pub` file |

The release workflow refuses to build if any of these are missing, because a release without them could never update.

### 3. Turn on the workflows

The workflow files are kept in `docs/ci/` because the assistant's GitHub connection may not create workflow files. On GitHub, on the `main` branch:

1. Open `docs/ci/release.yml`, choose the pencil (**Edit**), and in the file name box at the top change the path to `.github/workflows/release.yml` (press Backspace at the start of the name to step out of `docs/ci/`). Commit the change.
2. Do the same for `docs/ci/core.yml`, making it `.github/workflows/core.yml`. It runs the tests on Windows and Linux for every change.

## Publishing a version

1. Make sure `version` in `app/Cargo.toml` is the new number and that the change is on `main`.
2. Push the tag. From any clone: `git tag v0.2.0 origin/main` then `git push origin v0.2.0`. (A coding assistant can do this step once the setup above is done.)
3. Watch **Actions > Release RestedRealm Companion**. It takes about fifteen minutes.
4. Open **Releases**, check the draft: it has `RestedRealm Companion_<version>_x64-setup.exe`, a `.sig` file and `latest.json`. Press **Publish release**.

The download button on restedrealm.com (`/account/companion`) always points at the latest published release.

## Before code signing

Until the installer is code signed, Windows shows "Windows protected your PC" for a downloaded installer. Choose **More info**, then **Run anyway**. Automatic updates are not affected: they are checked with the updater key, and the app installs them itself.

### Free code signing through SignPath Foundation

SignPath Foundation signs open-source projects without charge. Applying needs the repository owner, so it cannot be done by an assistant. What to do:

1. Read the conditions and apply at https://signpath.org (the open-source programme). The project fits their usual conditions: MIT licence, public repository, built on GitHub Actions, no malware, and releases published from CI.
2. They ask for a short project description, the repository, who is allowed to request signing (the owner), and how releases are made. This document and `docs/COMPANION-APP.md` can be linked.
3. Expect them to ask for a code signing policy page on the project (who signs, what gets signed, a privacy link) and for signing to happen inside the release workflow through their GitHub integration. Once approved, the release workflow gets one extra signing step; ask the assistant to add it with the details SignPath gives you.
4. The publisher shown by Windows will be "SignPath Foundation", not RestedRealm.

Acceptance is their decision and can take a while. Everything else works without it.
