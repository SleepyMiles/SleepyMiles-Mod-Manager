# SleepyMiles Mod Manager

A Lethal Company mod manager that gets mods from [Thunderstore](https://thunderstore.io/c/lethal-company/) **and** from [SleepyMiles' mod site](https://sleepymiles.github.io/miles-lc-mods/), for mods that can't go on Thunderstore.

It is a fork of **[Gale](https://github.com/Kesomannen/gale) by Kesomannen**. Almost everything here is Gale's work: the interface, profiles, config editor, import/export and launching. Please [support Gale's author](https://ko-fi.com/kesomannen).

## What's different from Gale

- **Two mod sources in one list.** SleepyMiles' mods show up next to Thunderstore mods, with their dependencies, updates, READMEs and changelogs.
- **Profile codes carry both.** Export a profile as a code, and a friend who imports it gets the Thunderstore mods *and* the site mods. (Codes with site mods only import in this app, not in Gale or r2modman.)
- **Lethal Company only.**
- **No profile sync.** Sync needs Gale's server, so it is turned off. Use profile codes.
- **Separate data folder** (`%APPDATA%\com.sleepymiles.modmanager`), so it never touches your Gale profiles.

## Install

Download the latest `..._x64-setup.exe` from [Releases](https://github.com/SleepyMiles/SleepyMiles-Mod-Manager/releases/latest) and run it. If Windows says it protected your PC, click **More info**, then **Run anyway**. After that, the app updates itself.

## How the site source works

The site ([SleepyMiles/miles-lc-mods](https://github.com/SleepyMiles/miles-lc-mods)) is a static GitHub Pages site that serves a package index in Thunderstore's format (`api/v1/package-listing-index.gz`). Mod zips are downloaded from that repo's GitHub Releases, tagged `<Name>-<version>`. In the code, the site takes the slot Gale uses for Hexium (`Backend::SleepyMiles`, see `src-tauri/src/thunderstore/backend.rs`).

## Publishing mods to the site

Click the **Upload** icon in the sidebar (under Modpack) and **Log in with GitHub**. Only the `SleepyMiles` account is accepted. Choose a mod zip: the app checks `manifest.json` the same way the site does, shows the mod, and **Publish** creates the `<Name>-<version>` release with the zip. The site's publish workflow adds it to the site and to the app's mod list a minute or two later. To update a mod, raise `version_number` and publish again.

Login uses GitHub's device flow with the "SleepyMiles Mod Manager" OAuth App (client ID in `src-tauri/src/site_upload.rs`). The code is in `src-tauri/src/site_upload.rs` and `src/routes/upload/+page.svelte`.

## Releasing a new version

1. Add a section to `CHANGELOG.md`.
2. On GitHub, open the `VERSION` file, click the pencil, change it to the new version (for example `1.0.1`), and **Commit changes**. That starts the build. (**Actions → Release → Run workflow** also works.)
3. After about 15–20 minutes the release appears with the installer, and installed copies update themselves.

The workflow needs the `TAURI_SIGNING_PRIVATE_KEY` repository secret (the updater signing key). Keep that key safe: without it, existing installs can't be updated.

## Building locally

Same as Gale: install [Rust](https://rustup.rs), Node.js and pnpm, then `pnpm install` and `pnpm tauri dev`.

## License

GPL-3.0, same as Gale. See [LICENSE.md](LICENSE.md).
