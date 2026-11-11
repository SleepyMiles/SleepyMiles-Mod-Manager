# Changelog

## 1.0.12

- Unused config: configs of mods that have no DLL of their own are kept, such as SoundAPI sound packs (`soundpack.*.cfg`, e.g. LethalResonance). A config now also counts as in use if any installed mod's folder or file carries the mod's name.

## 1.0.11

- **Send logs** is now also on the upload page (cloud icon), above Miles's "Publish a mod" section.
- **Unused config** in File → Free up space. It works on the open profile: click **Check**, untick anything you want to keep, then **Remove**. A backup of the config folder is saved first. It finds:
  - config files of mods that are no longer in the profile (turned-off mods count as still there);
  - LethalLevelLoader settings for interiors and moons you removed, and JLL's small per-interior files for them. LethalLevelLoader only fills these in once a lobby loads, so they're only checked after you've hosted or joined a lobby. Otherwise it tells you to do that first and removes nothing.

## 1.0.10

- **Send logs to Miles** (File or Help menu): sends your BepInEx log, Lethal Company's Player.log (and the one from before the last restart), the app's log and your mod list straight to Miles's Discord, with your name and what happened. "Save as zip instead" puts the same zip in your Downloads folder.
- **Free up space** (File menu) replaces the two "Clear mod cache" items and shows what each option really frees:
  - **Unused mod files**: old versions left after updates, uninstalled mods, mods from deleted profiles and leftover profile folders. Safe to delete.
  - **Whole mod cache**: explained properly. Your profiles share these files, so it only frees the unused part, and mods download again next time.
  - **Gale's leftover files**: deletes what Gale left behind after you switched. It warns you if Gale is still installed and won't run while Gale is open.
- Deleting a profile now also deletes its mods' downloads when no other profile uses them, so the space is really freed, and tells you how much.

## 1.0.9

- Renamed to **Miles Mod Manager**. The program is now `Miles.exe` and the desktop and Start menu shortcuts are called **Miles**.
- New icon: the wooden crate with the MILES label, the tools and Miles himself. It shows on the taskbar, shortcuts, window, menu bar and About dialog.
- Updating from 1.0.8 or older moves you over automatically: your profiles, settings and GitHub login stay, the old shortcuts are replaced with "Miles" ones, a taskbar pin keeps working, and the old SleepyMiles Mod Manager program folder is removed.

## 1.0.8

- Window title, desktop shortcuts and messages say SleepyMiles Mod Manager instead of Gale.
- If Windows blocks the app from creating its folders on first launch (usually antivirus such as Norton CyberCapture), the error now says so and what to do.

## 1.0.7

- Mods published to SleepyMiles' site show up on the next **File → Fetch mods**, without waiting up to 10 minutes for GitHub's cache.

## 1.0.6

- Upload page in the sidebar (cloud icon, under Modpack) for publishing mods to SleepyMiles' site.
- "Log in with GitHub" instead of pasting a token. Only the SleepyMiles account can upload.
- Removed the hidden Ctrl+Alt+U upload window from 1.0.5.

## 1.0.5

- Upload mods to SleepyMiles' site from the app (Ctrl+Alt+U): pick a zip, check it, publish. Needs a GitHub token with write access to the site's repository.

## 1.0.4

- New app icon in the style of SleepyMiles' mod posters, and a matching small icon next to the File menu.

## 1.0.3

- New default colors: Neutral (primary) and Amber (accent).
- Browse page has a source picker: All sources, Thunderstore, or SleepyMiles' site.

## 1.0.2

- Fixed the app closing immediately on first launch (it tried to open a game that was removed).

## 1.0.1

- Same as 1.0.0 (still has the launch crash).

## 1.0.0

First release of SleepyMiles Mod Manager, based on [Gale](https://github.com/Kesomannen/gale) 1.23.3.

- Mods come from Thunderstore **and** [SleepyMiles' mod site](https://sleepymiles.github.io/miles-lc-mods/), in one list.
- Profile codes include mods from both, so friends get everything from one code.
- Lethal Company only.
- Profile sync is turned off (it needs Gale's own server). Use profile codes instead.
- Updates itself from this repository's releases.

Gale's own history is in [GALE-CHANGELOG.md](GALE-CHANGELOG.md).
