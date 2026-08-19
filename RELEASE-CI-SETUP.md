# Release CI setup — one-time, Cal only

`release-flatpak.yml` needs three repo secrets to build and publish releases. Claude can't
set these — they involve your GPG private key (and its passphrase) and a token with push
access to another repo, none of which should pass through a session transcript.

Run these from `~/Projects/zerkalo`. Requires `gh` to already be authenticated (it is, on
this machine).

## 1. `FLATPAK_GPG_PRIVATE_KEY`

Exports your signing key (armored/text form) straight into the secret — nothing touches disk:

```bash
gpg --export-secret-keys --armor A2918A9B43B199ADF9879F934AC9D5173DE4BC41 \
  | gh secret set FLATPAK_GPG_PRIVATE_KEY --repo calstfrancis/zerkalo
```

**The key has a passphrase** (confirmed 2026-08-19 — the first real run failed exactly here).
Importing it is fine on its own, but signing later needs it unlocked, and there's nothing to
prompt for non-interactively in CI — so the workflow presets the passphrase into the agent's
cache once, up front (`gpg-agent --preset`), rather than trying to feed it interactively per
signing call:

```bash
gh secret set FLATPAK_GPG_PASSPHRASE --repo calstfrancis/zerkalo
# paste the passphrase, Ctrl-D
```

## 2. `FLATPAK_REPO_TOKEN`

A token the workflow uses to push the built flatpak into `calstfrancis/flatpak`. Scope it as
narrowly as GitHub allows — a fine-grained PAT limited to just that one repo:

1. https://github.com/settings/personal-access-tokens/new
2. **Resource owner:** calstfrancis · **Repository access:** Only select repositories → `flatpak`
3. **Permissions:** Repository → Contents → **Read and write** (nothing else needed)
4. Generate, copy the token, then:

```bash
gh secret set FLATPAK_REPO_TOKEN --repo calstfrancis/zerkalo
# paste the token, Ctrl-D
```

## Testing before a real release

`release-flatpak.yml` also accepts manual dispatch, so you can test the whole pipeline
without cutting a real version tag:

```bash
gh workflow run release-flatpak.yml --repo calstfrancis/zerkalo -f version=0.24.5
```

(Use whatever version is currently in `Cargo.toml` — the workflow checks it matches.) Watch
it at https://github.com/calstfrancis/zerkalo/actions/workflows/release-flatpak.yml — a
manual-dispatch run skips the "CI must have passed" gate (there's no fresh commit to check
against) but does everything else for real, including the actual push to the public flatpak
repo. Worth doing once before relying on this for a real release.

## Rolling this out to the other seven flatpak apps

This setup is Zerkalo-only for now. The same three secrets, the same workflow shape (just
swapping the manifest path and app ID), would need repeating per-app — worth doing as a
shared reusable workflow (`workflow_call`) rather than copy-pasted eight times, if/when you
want the rest of the suite on this too.
