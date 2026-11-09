# Changelog

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
