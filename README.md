<img src="assets/icons/hicolor/128x128/apps/bing-wallpaper-manager.png" alt="" width="96" align="right">

# Bing Wallpaper Manager

**English** | [日本語](README.ja.md)

A desktop app for Ubuntu that lets you browse Bing’s daily photos and set them as your GNOME wallpaper. Built with Rust, [GPUI](https://gpui.rs) and [gpui-kit](https://gpui-kit.com).

![The selected photo fills the top of the rounded window; the title bar appears over it while the mouse is on the photo (English, dark theme)](docs/screenshot.jpg)

The photo you pick fills the top of a rounded, frameless window. Hover over it to bring up the title bar; set it as the wallpaper and it is marked *Current wallpaper*.

## Features

- **Dashboard**: the selected photo is shown large at the top, with *Today’s photos* and *Earlier photos* (grouped by date) below. The large photo stays pinned above the list, shrinks to a slim bar while you scroll, and expands again when you pick a photo or click the slim bar. Switching photos crossfades smoothly once the new image has loaded
- **Frameless window**: rounded corners, with the large photo reaching the top edge. The title bar (search, favorites, refresh, settings, full screen and the window buttons) appears over the photo only while the mouse is on it; drag the photo to move the window, double-click the top edge to maximize
- **Today’s photos**: the latest photo from the regions you choose, grouped by photo with the regions that show it. Six regions are on by default (Japan, United States, United Kingdom, Germany, France, China); Canada (English / French), India, Spain, Italy and Brazil can be turned on in the settings. While the app stays open it checks for new photos every hour
- **Set as wallpaper**: sets the GNOME wallpaper for both light and dark styles. The current wallpaper is marked *Current wallpaper*
- **Favorites**: mark photos with the ☆ button, the right-click menu or Ctrl+D; favorites get a ★ badge, and the ★ button in the title bar shows only your favorites (can be combined with search)
- **Earlier photos**: saved photos load continuously as you scroll. The official Bing API goes back 15 days. For older photos, *Load older photos* at the end of the list loads about a month at a time from a community archive ([Bing-Wallpaper-Archive](https://github.com/npanuhin/Bing-Wallpaper-Archive); the US since 2010, other regions since April 2024). Older photos are saved as thumbnails only; the full image is downloaded when you set it as the wallpaper
- **Search** (Ctrl+F) through saved photos
  - By words: titles, places, descriptions, regions and dates (such as `2026-11-08`). Titles and descriptions are saved in every available language, so “penguin”, “Pinguin” and “ペンギン” all find the same photo
  - By color: pick red, orange, yellow, green, cyan, blue, purple, pink, white or black to list photos with the most of that color first. Can be combined with words
  - Press Escape to close the search
- **Fit**: choose how the photo fits when its shape differs from the monitor, with a preview in the monitor’s proportions (zoom, show the whole photo, stretch, center at original size, span across monitors)
- **Photo info** (ⓘ on the large photo, Ctrl+I): a side panel with the title, place, date, regions and Bing’s description of the photo, plus *Look up on Bing* and *Copy title and location*
- **Photo actions**: right-click a photo or use the *…* button on the large photo to set it as the wallpaper, add it to your favorites, see its info, look it up on Bing, copy its title and location, open the image, show it in its folder, or delete it
- **Go to month**: jump straight to a month of the earlier photos
- **Delete** (Delete key): removes a photo from the folder after a confirmation; deleted photos are not downloaded again
- **Automatic updates**: a switch in the settings changes the wallpaper for you even when the app is closed — to each new photo of the day, or to a different favorite or saved photo at an interval you pick
- **Settings** (☰ at the top right, Ctrl+,): General (language, theme), Regions, Wallpaper (fit), Automatic updates, Storage (folder and disk usage), and About (version, license, libraries used)
- **11 languages**: Japanese, English, Chinese (Simplified / Traditional), Korean, German, French, Spanish, Italian, Portuguese and Russian. *Automatic* follows the OS language (`LANG`). Photo titles and descriptions prefer the version shown in a region that uses the selected language
- **Theme**: match the system, light or dark
- **Keyboard**: ← → to choose a photo, Enter to set it as the wallpaper, Ctrl+D to add or remove a favorite, Ctrl+I for photo info, Delete to delete, F5 / Ctrl+R to refresh, Ctrl+F to search, Ctrl+, for settings, F11 for full screen
- **Command line**: `--apply` sets today’s photo (or, with `--random`, a saved photo) as the wallpaper without opening the window
- **One window**: starting the app again brings the open window forward instead of opening a second one, and the window opens at the size you left it

## Browsing

Scroll the list and the large photo shrinks to a slim bar, leaving more room for photos; pick a photo or click the bar to expand it again. Earlier photos are grouped by date, and favorites carry a ★ badge. Right-click any photo (or use *…* on the large photo) to set it as the wallpaper, add it to your favorites, see its info, look it up on Bing, copy its title and location, open the image, show it in its folder or delete it.

![Scrolled list with the slim bar at the top and a photo’s right-click menu](docs/browse.jpg)

### Go to month

Once you have photos from more than one month, **Go to month** at the right of *Earlier photos* lists the months you have saved (grouped by year when there are several years). Pick one and the list scrolls to the first photo of that month.

![The Go to month menu listing the saved months](docs/jump.jpg)

### Deleting a photo

Choose **Delete…** in a photo’s menu, or press the Delete key for the photo shown at the top. After you confirm, the image, its thumbnail and its details are removed from the photo folder. Deleted photos are remembered, so refreshing or loading older photos does not bring them back. Today’s photos and the current wallpaper cannot be deleted.

![Confirmation before deleting a photo](docs/delete.jpg)

## Photo info

Click **ⓘ** on the large photo (or press **Ctrl+I**, or choose *Photo info* in a photo’s menu) to open a panel with the photo’s title, place and credit, date, the regions that showed it, and Bing’s description in your language when there is one. Descriptions of recent photos are fetched together with the photos; for older photos they come from the community archive. *Look up on Bing* opens Bing’s page about the photo in your browser, and *Copy title and location* puts them on the clipboard. Press Escape to close the panel.

![The photo info panel with the description of the photo](docs/info.jpg)

## Search

Press **Ctrl+F** (or click the 🔍 button in the title bar) to search the photos you have saved. While searching, the list shows only the matching photos; click one to see it large at the top, or press Escape to go back.

### By words

Type any word from a photo’s title, place, description, region or date (such as `2026-11-08`). Titles and descriptions are saved in every language the photo was shown in, and the longer descriptions from the community archive are searched too, so a word in one language also finds photos shown in other regions.

![Searching for “autumn”](docs/search-words.jpg)

### By color

Pick one of ten colors (red, orange, yellow, green, cyan, blue, purple, pink, white, black) to list the photos that contain the most of that color first. The color shares are computed from each photo’s thumbnail in the background. Words and a color can be combined, for example “mountain” + blue.

![Photos with a lot of blue](docs/search-color.jpg)

## Wallpaper fit

When the photo’s shape differs from your monitor, choose how it fits in **Settings** (☰ at the top right, or Ctrl+,) → **Wallpaper**. The preview uses your monitor’s proportions and the photo shown at the top; with *Show the whole photo*, the leftover space is filled with the background color GNOME uses. The current wallpaper updates right away.

![Wallpaper settings previewing “Show the whole photo”](docs/settings-fit.jpg)

## Regions

Bing shows different photos, titles and descriptions in different regions. In **Settings** → **Regions**, choose which regions the dashboard gets photos from: Japan, United States, United Kingdom, Germany, France and China are on by default, and Canada (English), Canada (French), India, Spain, Italy and Brazil can be added. At least one region stays on. Regions where Bing only shows its worldwide photo are not listed.

![Region settings with a checkbox for each Bing region](docs/regions.jpg)

## Automatic updates

Turn on **Settings** → **Automatic updates** to have the wallpaper change by itself, even when the app is closed. Choose what to use:

- **Today’s photo**: checks every hour (and shortly after you log in) and sets the new photo when one is published, for the region you pick. A wallpaper you choose yourself stays until the next photo arrives
- **From favorites**: picks a different favorite each time, every 30 minutes, hour, 3 hours, 6 hours or day
- **From saved photos**: the same, from everything you have saved. Older photos are downloaded when they are picked

The switch creates a systemd user timer (`~/.config/systemd/user/bing-wallpaper-manager.timer` and `.service`) that runs this app with `--apply`, and turning it off removes the timer. The timer runs the app from where it is now, so turn the switch off and on again if you move the app.

![Automatic update settings with the switch turned on](docs/auto.jpg)

## Built with

The user interface is built on these two open-source projects:

- **[GPUI](https://gpui.rs)** — the GPU-accelerated UI framework developed by [Zed Industries](https://zed.dev) for the Zed editor ([source](https://github.com/zed-industries/zed), Apache-2.0). This app uses the snapshot published on crates.io as `gpui-pre`
- **[gpui-kit](https://gpui-kit.com)** — a component library and toolkit for GPUI by [Longbridge](https://github.com/longbridge) ([source](https://github.com/longbridge/gpui-kit), Apache-2.0). The buttons, dialogs, menus, inputs, selects, scrollbars, title bar and theming come from its `gpui-component` and `gpui-base` crates

Thanks to both projects. All other libraries are listed in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

## Installation

### Requirements

- Ubuntu with the GNOME desktop (developed on Ubuntu 26.04; Wayland and X11). The wallpaper is set through GNOME’s `gsettings`
- A graphics driver with Vulkan support (included in a normal Ubuntu desktop install)

### Prebuilt binary (x86_64)

Each [release](https://github.com/TomoyukiNakama/bing-wallpaper-manager/releases/latest) has a prebuilt package for Ubuntu 26.04 or newer (glibc 2.43+). Download `bing-wallpaper-manager-<version>-x86_64-linux.tar.gz` and extract it into `~/.local`:

```sh
tar -xzf bing-wallpaper-manager-*-x86_64-linux.tar.gz -C ~/.local --strip-components=1
```

To build from source instead, follow the steps below.

### 1. Install the build tools

```sh
sudo apt install git curl build-essential
```

A C compiler (`build-essential`) is needed because one of the font libraries is built from C sources.

### 2. Install Rust

Install the current stable Rust with [rustup](https://rustup.rs). The Rust packaged with Ubuntu 26.04 (1.93) is too old for GPUI.

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"   # or open a new terminal
```

If you prefer to keep Ubuntu’s own `cargo` on your PATH, that works too: `.cargo/config.toml` points cargo at `scripts/rustc-stable`, which runs the rustc of your rustup stable toolchain (and falls back to the `rustc` on your PATH when rustup is not installed).

### 3. Get the source

```sh
git clone https://github.com/TomoyukiNakama/bing-wallpaper-manager.git
cd bing-wallpaper-manager
```

### 4. Build and install

```sh
scripts/install.sh
```

This builds the release version and installs the app, its icon and a desktop entry under `~/.local`. *Bing Wallpaper* then appears in the GNOME app grid (log out and back in if it does not show up right away), and `bing-wallpaper-manager` can be started from a terminal if `~/.local/bin` is on your PATH. The first build downloads and compiles all dependencies and takes several minutes.

To install somewhere else, set `PREFIX`, for example `PREFIX=/opt/bing-wallpaper scripts/install.sh`.

### Update

```sh
git pull
scripts/install.sh
```

`scripts/install.sh` always rebuilds before installing, so running it again after `git pull` installs the new version.

### Uninstall

```sh
scripts/install.sh --uninstall
```

This removes the app, icon and desktop entry. Your settings (`~/.config/bing-wallpaper-manager/`) and saved photos (the storage folder) are kept; delete them yourself if you no longer need them.

### Run without installing

```sh
cargo run --release
```

## Storage

Choose the storage folder under *Settings → Storage* (default: `~/Pictures/BingWallpaper`, or the localized Pictures folder). Settings are saved to `~/.config/bing-wallpaper-manager/config.json`, and things the app remembers (such as the window size) to `state.json` in the same folder.
A photo is saved only once even if it is shown in several regions or on different days, and saved images are never downloaded again.

```text
<storage folder>/
  20261108_OHR.Foo_UHD.jpg              Wallpaper image (UHD)
  .bing-wallpaper/meta/OHR.Foo.json     Metadata: titles, descriptions, regions, color shares
  .bing-wallpaper/thumbs/OHR.Foo.jpg    Thumbnail (640x360)
  .bing-wallpaper/previews/OHR.Foo.jpg  Medium-size image for showing older photos large (1920x1080)
  .bing-wallpaper/deleted.txt           Photos you deleted (so they are not downloaded again)
```

## Command line

```sh
bing-wallpaper-manager --apply                       # set today’s photo as the wallpaper
bing-wallpaper-manager --apply --mkt en-US           # choose the region
bing-wallpaper-manager --apply --if-new              # do nothing if it is the same photo as last time
bing-wallpaper-manager --apply --random              # set a random saved photo
bing-wallpaper-manager --apply --random --favorites  # set a random favorite
```

Without `--mkt`, the region chosen under *Automatic updates* is used (or the first region shown). If the network is not up yet, for example right after logging in, `--apply` retries a few times before giving up.

### Update every day

Use **Settings** → **Automatic updates** (see [Automatic updates](#automatic-updates)); it sets up the systemd user timer for you. To check on it:

```sh
systemctl --user list-timers bing-wallpaper-manager.timer
journalctl --user -u bing-wallpaper-manager.service
```

If you created `bing-wallpaper.timer` by hand with an earlier version, disable it so the two do not both run: `systemctl --user disable --now bing-wallpaper.timer`.

## Development

### Screenshots

`scripts/screenshot.sh` runs the app on a virtual Wayland display (sway in headless mode) and saves a PNG.
It does not touch your desktop, settings, storage folder, wallpaper or systemd timers (screenshot data lives in `target/screenshot-fixture/`).

```sh
sudo apt install sway grim          # first time only
cargo build --release
scripts/screenshot.sh target/shot.png            # default: wait 15 s, 1440x960
BWM_THEME=dark BWM_LANG=de scripts/screenshot.sh target/dark.png 10 1280x1300
```

`BWM_THEME=light|dark` fixes the theme and `BWM_LANG=de` (etc.) fixes the language; both also work in a normal launch.
`FLOAT=1280x840` shows the window floating instead of filling the screen (to check the rounded corners and window buttons), and `BG=<image>` puts an image behind it as the desktop background.

Put clicks, right-clicks, drags, scrolling, keys and typing in `ACTIONS` to interact before the screenshot. Input is sent through a virtual mouse and keyboard in `tools/vpointer` (built automatically on first use).

```sh
ACTIONS=$'key ctrl-f\ntype penguin\nsleep 3\nshot target/step1.png' \
  scripts/screenshot.sh target/step2.png 10 1280x1000
```

The images in `docs/` are taken this way with the dark theme (English for `README.md`, Japanese `*.ja.jpg` for `README.ja.md`) and saved as JPEG at quality 85, for example:

```sh
BG=target/screenshot-fixture/store/20261112_OHR.AbrahamCanada_UHD.jpg FLOAT=1280x840 BWM_LANG=en BWM_THEME=dark \
  ACTIONS=$'click 583 660\nsleep 1.5\nkey enter\nsleep 2\nclick 720 250\nsleep 1.5' \
  scripts/screenshot.sh target/main.png 12 1440x980
```

### Release packages

`scripts/package-release.sh` builds the release version and creates `target/dist/bing-wallpaper-manager-<version>-x86_64-linux.tar.gz` (and its `.sha256`) with the binary, icons, desktop entry and license files, including the license texts of all bundled libraries. The version comes from `Cargo.toml`.

### Translations

Strings live in `locales/<language>.yml` (`ja`, `en`, `zh-CN`, `zh-TW`, `ko`, `de`, `fr`, `es`, `it`, `pt-BR`, `ru`).

- Write placeholders as `%{count}`
- Every file must have the same keys (`cargo test` checks this). Missing translations fall back to English
- To add a language, add a file under `locales/` and a variant to `Language` in `src/i18n.rs`

### Layout

| File | Role |
| --- | --- |
| `src/main.rs` | Dashboard UI and CLI |
| `src/bing.rs` | Bing HPImageArchive API and downloads |
| `src/archive.rs` | Older photos and descriptions from the community archive |
| `src/library.rs` | Saving and loading photos in the storage folder |
| `src/search.rs` | Search by words and colors |
| `src/colors.rs` | Color share analysis of photos |
| `src/settings.rs` | Settings dialog |
| `src/info.rs` | Photo info panel |
| `src/schedule.rs` | Automatic updates (systemd user timer) |
| `src/single.rs` | Keeps a single window (a second launch brings it forward) |
| `src/config.rs` | Settings file |
| `src/theme.rs` | Colors (light / dark) and font |
| `src/i18n.rs` | Language switching and date formats (strings are referenced with `tr!("key")`) |
| `src/wallpaper.rs` | Setting the GNOME wallpaper with `gsettings` |
| `locales/*.yml` | Translations (embedded at build time with rust-i18n) |
| `src/third_party.tsv` / `THIRD_PARTY_LICENSES.md` | Libraries used (generated by `scripts/gen-third-party.py`) |
| `assets/icons/` | App icon (SVG, and PNGs rendered by `scripts/render-icon.py`) |
| `assets/bing-wallpaper-manager.desktop` | Desktop entry (installed by `scripts/install.sh`) |
| `scripts/rustc-stable` | Picks the rustup stable rustc for cargo (see Build) |
| `scripts/screenshot.sh` | Screenshots on a virtual display |
| `tools/vpointer/` | Virtual mouse and keyboard for screenshots (separate crate) |

## License

Licensed under either the [MIT License](LICENSE-MIT) or the [Apache License 2.0](LICENSE-APACHE), at your option (MIT OR Apache-2.0). Libraries used are listed in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) (regenerate with `python3 scripts/gen-third-party.py` after changing dependencies).

Photos are copyrighted by their photographers and providers. This app is not affiliated with Microsoft.
