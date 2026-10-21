<p align="center">
  <b>minimalist terminal soundcloud & audio player for linux</b>
  <br>
  <i>fast streaming · zero bloat · @ttype aesthetic · rust</i>
</p>

<p align="center">
  <img src="https://img.shields.io/github/license/laguser/soundcloud-tui?style=flat-square&color=blue" alt="License">
  <img src="https://img.shields.io/badge/platform-linux-lightgrey?style=flat-square" alt="Linux">
  <img src="https://img.shields.io/badge/ui-ratatui-orange?style=flat-square" alt="Ratatui">
</p>

<p align="center">
  <a href="#english">English</a> • <a href="#русский">Русский</a>
</p>

---

<a name="english"></a>
## English

A distraction-free, keyboard-centric terminal SoundCloud & audio stream player designed natively for Linux. Rewritten entirely in Rust with the signature aesthetic of [@ttype](https://github.com/laguser/ttype): clean transparent typography, conveyor-tape progress bars, non-blocking audio engine via `mpv`, and instant search.

### Features
- **Clean & Transparent Aesthetic:** Zero clunky borders or heavy frameworks. Blends directly into your terminal background and color scheme.
- **Native Linux Architecture:** High-performance Rust backend using asynchronous `mpv` IPC sockets for instant playback and minimal memory footprint.
- **SoundCloud + Fallback Search:** Instant track search powered by `yt-dlp`, with direct URL streaming support (tracks, playlists, or YouTube fallbacks).
- **Interactive Player Tape:** Center-aligned progress conveyor tape with customizable visual styles (`tape`, `minimal`, `blocky`).
- **Views & History:**
  - `F1` / Search: Fast inline query or direct SoundCloud track/playlist link.
  - `F2` / Queue: On-the-fly queue management (add, reorder, delete).
  - `F3` / Favorites: One-key liking (`l`) with persistent local storage.
  - `F4` / History: Automatic playback history tracking.
  - `F5` / Settings: On-the-fly theme switcher (`catppuccin`, `tokyo night`, `nord`, `gruvbox`, `dracula`, `amber`, `white`, `auto`).
- **Zero Python/Venv Headache:** Single standalone static binary.

### Requirements
- `mpv`
- `yt-dlp`

Install on Arch Linux:
```bash
sudo pacman -S mpv yt-dlp
```

Install on Debian/Ubuntu:
```bash
sudo apt install mpv yt-dlp
```

### Installation

#### Pacman / Arch Linux (AUR)
```bash
# Using an AUR helper (paru / yay):
paru -S soundcloud-tui-git
# Or build locally from PKGBUILD:
git clone https://github.com/laguser/soundcloud-tui.git
cd soundcloud-tui
makepkg -si
```

#### Via Cargo (Recommended)
```bash
cargo install soundcloud-tui
```

#### From Source
Requires Rust (`cargo`):
```bash
git clone https://github.com/laguser/soundcloud-tui.git
cd soundcloud-tui
cargo build --release
install -m 755 target/release/sctui ~/.local/bin/
```

### Keybindings
| Key | Action |
| :--- | :--- |
| `space` | Play / Pause playback |
| `n` / `p` | Next / Previous track in queue |
| `a` | Add highlighted track to Queue |
| `d` | Remove track from Queue / Favorites |
| `l` | Like / unlike current track (add to Favorites) |
| `s` | Toggle Shuffle |
| `r` | Cycle Repeat (Off ➔ All ➔ One) |
| `y` | Synchronized Lyrics karaoke view |
| `S` | Sync personal SoundCloud library (likes & playlists) |
| `←` / `→` (or `b` / `f`) | Seek -5s / +5s |
| `+` / `-` | Volume up / down |
| `i` or `/` | Focus search bar |
| `tab` | Cycle views (`search` ➔ `queue` ➔ `favorites` ➔ `history` ➔ `settings` ➔ `lyrics`) |
| `f1` - `f6` | Jump directly to view |
| `esc` | Clear search / Back to search screen |
| `esc esc` / `ctrl+c` | Exit |

### Desktop Integration (MPRIS2 & Caelestia Shell)
`sctui` natively registers as an MPRIS2 D-Bus media player (`org.mpris.MediaPlayer2.soundcloud-tui`).
This provides seamless real-time integration with:
- **Caelestia Shell** & Waybar media widgets
- System media keys (`XF86AudioPlay`, `XF86AudioNext`, etc.)
- Notification popups with album artwork, title, artist, and duration

---

<a name="русский"></a>
## Русский

Минималистичный терминальный SoundCloud плеер под Linux, полностью переписанный на Rust в стиле [@ttype](https://github.com/laguser/ttype). Без тяжёлых фреймворков, без `venv` и зависимостей Python — быстрый запуск, чистый прозрачный TUI интерфейс, стриминг аудио через легковесный `mpv` IPC сокет, синхронизированные караоке-тексты песен и поддержка системных медиа-виджетов (Caelestia Shell, Waybar, MPRIS2).

### Особенности
- **Стиль и эстетика @ttype:** Чистый интерфейс с прозрачным фоном, адаптирующийся под цвета терминала, кастомный бегущий прогресс-бар (лента).
- **MPRIS2 D-Bus интеграция:** Полная привязка к **Caelestia Shell**, Waybar и медиа-клавишам системы (отображение обложек, названий, управление воспроизведением).
- **Синхронизированные слова песен (Караоке):** Нажмите `y` для открытия экрана синхронизированных текстов (LRCLIB API) с плавной подсветкой текущей строки.
- **Синхронизация библиотеки SoundCloud:** Введите `oauth_token` в `~/.config/soundcloud-tui/config.json` и нажимайте `S` для автоматической загрузки ваших лайков и плейлистов.
- **Полноценно под Linux:** Построен на Rust с асинхронным сокетом mpv — мгновенный отклик, никакой нагрузки на процессор и память.
- **Поиск SoundCloud и прямые ссылки:** Быстрый поиск через `yt-dlp`, поддержка вставки прямых ссылок на треки/плейлисты SoundCloud, а также резервный поиск.

### Системные требования
- `mpv` (для воспроизведения аудио)
- `yt-dlp` (для извлечения аудиопотоков)

Установка в Arch Linux:
```bash
sudo pacman -S mpv yt-dlp
```

### Установка

#### Через Pacman / AUR (Arch Linux):
```bash
# Через AUR-хелпер (paru / yay):
paru -S soundcloud-tui-git

# Или сборка из репозитория:
git clone https://github.com/laguser/soundcloud-tui.git
cd soundcloud-tui
makepkg -si
```

#### Через Cargo:
```bash
cargo install soundcloud-tui
```

#### Сборка из исходников:
```bash
git clone https://github.com/laguser/soundcloud-tui.git
cd soundcloud-tui
cargo build --release
install -m 755 target/release/sctui ~/.local/bin/
```

Запуск:
```bash
sctui
```

