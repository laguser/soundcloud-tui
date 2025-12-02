from typing import List, Dict, Optional
import platform
import asyncio
import subprocess
import sys
import os
import tempfile
import shutil
import time
import json
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

# Автоустановка зависимостей
REQUIRED = ["textual", "yt_dlp", "requests"]

missing = []
for pkg in REQUIRED:
    try:
        __import__(pkg)
    except ImportError:
        missing.append(pkg)

if missing:
    print("\n❌ Не установлены библиотеки:")
    for m in missing:
        print(" -", m)
    print("\n✅ Установи их вручную:")
    print("pip install textual pygame-ce yt-dlp requests")
    print("\n⚠ На Linux обязательно запускать из venv:")
    print("source venv/bin/activate")
    sys.exit(1)

import yt_dlp
import requests
from textual.app import App, ComposeResult
from textual.widgets import Header, Footer, Input, ListView, ListItem, Label, Static, ProgressBar

try:
    import pygame.mixer as mixer
except Exception:
    from pygame import mixer

mixer.init(frequency=44100, size=-16, channels=2, buffer=512)

APP_DIR = Path.cwd()
HISTORY_FILE = APP_DIR / "history.json"
IS_LINUX = platform.system().lower() == "linux"
COOKIES_PATH = APP_DIR / "cookies.txt"

temp_dir: Optional[str] = None
current_file: Optional[str] = None


def format_duration(seconds) -> str:
    try:
        m, s = divmod(int(seconds or 0), 60)
        return f"{m}:{s:02d}"
    except Exception:
        return "0:00"


def load_history() -> List[Dict]:
    try:
        if HISTORY_FILE.exists():
            with HISTORY_FILE.open("r", encoding="utf8") as f:
                data = json.load(f)
                if isinstance(data, list):
                    return data
    except Exception:
        pass
    return []


def save_history(history: List[Dict]) -> None:
    try:
        tmp = HISTORY_FILE.with_suffix(".json.tmp")
        with tmp.open("w", encoding="utf8") as f:
            json.dump(history, f, ensure_ascii=False, indent=2)
        tmp.replace(HISTORY_FILE)
    except Exception:
        pass


def append_history_item(item: Dict) -> None:
    hist = load_history()
    entry = {
        "id": item.get("id"),
        "title": item.get("title") or item.get("url") or "Unknown",
        "artist": item.get("artist") or item.get("user", {}).get("username") or "",
        "url": item.get("url"),
        "ts": int(time.time()),
    }
    hist = [h for h in hist if not (h.get("url") == entry["url"] and entry["url"])]
    hist.insert(0, entry)
    hist = hist[:500]
    save_history(hist)


def has_ffmpeg() -> bool:
    from shutil import which
    return which("ffmpeg") is not None or which("avconv") is not None


def ensure_temp_dir() -> str:
    global temp_dir
    if temp_dir is None:
        temp_dir = tempfile.mkdtemp(prefix="sc_tui_")
    return temp_dir


def cleanup_temp_dir() -> None:
    global temp_dir
    if temp_dir and os.path.isdir(temp_dir):
        try:
            shutil.rmtree(temp_dir)
        except Exception:
            pass
    temp_dir = None


def cleanup_old_files(max_age_seconds: int = 3600) -> None:
    d = ensure_temp_dir()
    now = time.time()
    try:
        for fn in os.listdir(d):
            fp = os.path.join(d, fn)
            try:
                if now - os.path.getmtime(fp) > max_age_seconds:
                    os.remove(fp)
            except Exception:
                pass
    except Exception:
        pass


def get_ydl_opts(outdir: str, use_ffmpeg: bool) -> dict:
    opts = {
        "format": "bestaudio/best",
        "outtmpl": os.path.join(outdir, "%(id)s.%(ext)s"),
        
        # Настройки для работы с SoundCloud
        "extractor_args": {
            "soundcloud": {
                "client_id": "iZIs9mchVcX5lhVRyQGGAYlNPVldzAoX",  # Важно для Linux
            }
        },
        
        "geo_bypass": True,
        "geo_bypass_country": "US",
        "prefer_ipv4": True,
        "force_ipv4": True,
        
        "nocheckcertificate": True,
        "ignoreerrors": True,
        
        "http_headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            "Accept-Language": "en-US,en;q=0.9",
            "Accept-Encoding": "gzip, deflate",
            "DNT": "1",
            "Connection": "keep-alive",
            "Upgrade-Insecure-Requests": "1",
            "Sec-Fetch-Dest": "document",
            "Sec-Fetch-Mode": "navigate",
            "Sec-Fetch-Site": "none",
        },
        
        "retries": 10,
        "fragment_retries": 10,
        "skip_unavailable_fragments": True,
        "no_part": True,
        "no_check_certificate": True,
    }

    # ✅ Специальные настройки для Linux
    if IS_LINUX:
        # Форсируем использование cookies если есть
        if COOKIES_PATH.exists():
            opts["cookiefile"] = str(COOKIES_PATH)
            print("✅ Linux: Использую cookies.txt")
        else:
            print("⚠ Linux: cookies.txt не найден, использую стандартный доступ")
            
        # Для Linux используем более надежные настройки
        opts["socket_timeout"] = 30
        opts["source_address"] = "0.0.0.0"
        opts["force_generic_extractor"] = False
        
        # Пробуем разные client_id для SoundCloud
        opts["extractor_args"]["soundcloud"] = {
            "client_id": [
                "iZIs9mchVcX5lhVRyQGGAYlNPVldzAoX",  # основной
                "LvWovRaJZlqN2qFgVUeJXzKwd8g209lA",   # резервный 1
                "e2f5a57c2d9d87a7ff6d7d81e8c6f8b7",  # резервный 2
                "02gUJC0hH2ct1EGOcYXQIzRFU91c72Ea"   # резервный 3
            ]
        }

    if use_ffmpeg:
        opts["postprocessors"] = [{
            "key": "FFmpegExtractAudio",
            "preferredcodec": "mp3",
            "preferredquality": "192",
        }]

    return opts


def download_track_file(url: str, outdir: Optional[str] = None) -> str:
    if outdir is None:
        outdir = ensure_temp_dir()
    use_ffmpeg = has_ffmpeg()
    ydl_opts = get_ydl_opts(outdir, use_ffmpeg)

    print(f"🌐 Начинаю загрузку: {url}")
    print(f"📁 Директория: {outdir}")
    print(f"🔧 FFmpeg: {'Есть' if use_ffmpeg else 'Нет'}")
    
    # Для отладки на Linux
    if IS_LINUX:
        print("🐧 Linux режим активен")
        if COOKIES_PATH.exists():
            print(f"🍪 Cookies файл: {COOKIES_PATH}")

    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            # Пробуем получить информацию без загрузки сначала
            info = ydl.extract_info(url, download=False)
            if not info:
                raise Exception("Не удалось получить информацию о треке")
                
            print(f"📊 Получена информация: {info.get('title', 'Unknown')}")
            print(f"🔗 Форматы доступны: {len(info.get('formats', []))}")
            
            # Теперь загружаем
            ydl.download([url])
            
            # Пытаемся найти скачанный файл
            try:
                filename = ydl.prepare_filename(info)
            except Exception:
                filename = None

        if filename and os.path.exists(filename):
            print(f"✅ Файл найден: {filename}")
            return filename

        file_id = info.get("id")
        if file_id:
            for fn in os.listdir(outdir):
                if fn.startswith(file_id):
                    found = os.path.join(outdir, fn)
                    print(f"✅ Найден по ID: {found}")
                    return found

        files = [os.path.join(outdir, f) for f in os.listdir(outdir) 
                if os.path.isfile(os.path.join(outdir, f))]
        if files:
            files.sort(key=lambda p: os.path.getmtime(p), reverse=True)
            print(f"✅ Найден последний файл: {files[0]}")
            return files[0]

        raise FileNotFoundError("Не удалось найти скачанный файл")

    except Exception as e:
        print(f"❌ Ошибка при загрузке: {e}")
        if IS_LINUX:
            print("🐧 Для Linux попробуйте:")
            print("1. Убедитесь, что yt-dlp обновлен: pip install -U yt-dlp")
            print("2. Создайте файл cookies.txt из браузера")
            print("3. Попробуйте запустить с VPN")
        raise


def get_track_full_info(track_url: str) -> Optional[Dict]:
    """Быстрое получение информации о треке"""
    ydl_opts = {
        "quiet": True,
        "skip_download": True,
        "no_warnings": True,
        
        # Критические настройки для SoundCloud
        "extractor_args": {
            "soundcloud": {
                "client_id": "iZIs9mchVcX5lhVRyQGGAYlNPVldzAoX"
            }
        },
        
        "geo_bypass": True,
        "geo_bypass_country": "US",
        "prefer_ipv4": True,
        "force_ipv4": True,
        "source_address": "0.0.0.0",
        
        "nocheckcertificate": True,
        "ignoreerrors": True,
        
        "http_headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "*/*",
            "Accept-Language": "en-US,en;q=0.9",
            "Referer": "https://soundcloud.com/",
            "Origin": "https://soundcloud.com",
        },
        
        "socket_timeout": 15,
        "extract_flat": False,
    }
    
    # Дополнительные настройки для Linux
    if IS_LINUX:
        if COOKIES_PATH.exists():
            ydl_opts["cookiefile"] = str(COOKIES_PATH)
            print("🍪 Использую cookies для получения информации")
        
        ydl_opts["extractor_args"]["soundcloud"] = {
            "client_id": [
                "iZIs9mchVcX5lhVRyQGGAYlNPVldzAoX",
                "LvWovRaJZlqN2qFgVUeJXzKwd8g209lA",
                "e2f5a57c2d9d87a7ff6d7d81e8c6f8b7"
            ]
        }

    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(track_url, download=False)
            if info:
                return {
                    "id": info.get("id"),
                    "title": info.get("title") or "Unknown",
                    "artist": info.get("uploader") or info.get("artist") or info.get("creator") or "",
                    "duration": info.get("duration") or 0,
                    "url": info.get("webpage_url") or track_url,
                }
    except Exception as e:
        print(f"❌ Ошибка получения информации: {type(e).__name__}: {e}")
        if IS_LINUX:
            print("🐧 Проблема на Linux. Пробую альтернативный метод...")
            # Пробуем альтернативный метод для Linux
            return get_track_info_alternative(track_url)
    return None


def get_track_info_alternative(track_url: str) -> Optional[Dict]:
    """Альтернативный метод получения информации для Linux"""
    try:
        # Используем curl для получения информации
        cmd = [
            "yt-dlp",
            "--skip-download",
            "--print-json",
            "--no-warnings",
            "--force-ipv4",
            track_url
        ]
        
        if COOKIES_PATH.exists():
            cmd.extend(["--cookies", str(COOKIES_PATH)])
        
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=30
        )
        
        if result.returncode == 0 and result.stdout:
            info = json.loads(result.stdout)
            return {
                "id": info.get("id"),
                "title": info.get("title") or "Unknown",
                "artist": info.get("uploader") or info.get("artist") or "",
                "duration": info.get("duration") or 0,
                "url": info.get("webpage_url") or track_url,
            }
    except Exception as e:
        print(f"❌ Альтернативный метод тоже не сработал: {e}")
    
    return None


def simple_playlist_extract(url: str, progress_callback=None) -> List[Dict]:
    """БЫСТРОЕ извлечение плейлиста с параллельной загрузкой"""
    
    # Проверяем, это SoundCloud или YouTube
    is_soundcloud = "soundcloud.com" in url or "snd.sc" in url
    
    ydl_opts_flat = {
        "quiet": True,
        "extract_flat": True,
        "force_ipv4": True,
        "prefer_ipv4": True,
        "no_warnings": True,
        
        "geo_bypass": True,
        "geo_bypass_country": "US",
        
        "nocheckcertificate": True,
        "ignoreerrors": False,
        
        "http_headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "*/*",
            "Accept-Language": "en-US,en;q=0.9",
        },
    }
    
    # Настройки для SoundCloud
    if is_soundcloud:
        ydl_opts_flat["extractor_args"] = {
            "soundcloud": {
                "client_id": "iZIs9mchVcX5lhVRyQGGAYlNPVldzAoX"
            }
        }
    
    # Для Linux добавляем cookies
    if IS_LINUX and COOKIES_PATH.exists():
        ydl_opts_flat["cookiefile"] = str(COOKIES_PATH)
        print("🍪 Использую cookies для плейлиста")

    try:
        print(f"📡 Получаю список треков...")
        with yt_dlp.YoutubeDL(ydl_opts_flat) as ydl:
            playlist_dict = ydl.extract_info(url, download=False)

            if not playlist_dict:
                print("❌ Не удалось получить информацию о плейлисте")
                return []

            # Если это один трек
            if playlist_dict.get("_type") != "playlist":
                print("ℹ️ Это один трек, загружаю...")
                info = get_track_full_info(url)
                return [info] if info else []

            # Получаем список URL треков
            entries = playlist_dict.get("entries", [])
            if not entries:
                print("❌ Плейлист пустой")
                return []

            track_urls = []
            print(f"📋 Найдено {len(entries)} треков в плейлисте")

            for i, entry in enumerate(entries, 1):
                if not entry:
                    print(f"  ⚠️ Трек {i} пропущен (пустая запись)")
                    continue

                # Для SoundCloud нужен особый подход
                if is_soundcloud:
                    track_url = entry.get("url")
                    if not track_url:
                        track_id = entry.get("id")
                        if track_id:
                            track_url = f"https://soundcloud.com/track-{track_id}"
                else:
                    track_url = (
                        entry.get("webpage_url") or
                        entry.get("url") or
                        entry.get("original_url")
                    )

                if not track_url:
                    continue

                if not track_url.startswith("http"):
                    if is_soundcloud:
                        track_url = f"https://soundcloud.com/{track_url.lstrip('/')}"
                    else:
                        track_url = f"https://youtube.com/watch?v={track_url}"

                track_urls.append(track_url)

            if not track_urls:
                print("❌ Не удалось получить URL треков")
                return []

            print(f"🚀 Загружаю информацию о {len(track_urls)} треках параллельно (10 потоков)...")

            # Параллельная загрузка информации
            tracks = []
            total = len(track_urls)
            completed = 0

            with ThreadPoolExecutor(max_workers=10) as executor:
                future_to_index = {executor.submit(get_track_full_info, url): (idx, url) 
                                 for idx, url in enumerate(track_urls)}

                results = [None] * len(track_urls)

                for future in as_completed(future_to_index):
                    idx, url = future_to_index[future]
                    completed += 1

                    try:
                        result = future.result()
                        if result:
                            results[idx] = result

                        if progress_callback:
                            progress_callback(completed, total)

                        if completed % 5 == 0 or completed == total:
                            print(f"  ⏳ {completed}/{total} треков обработано...")
                    except Exception as e:
                        print(f"  ❌ Ошибка при обработке трека {idx + 1}: {e}")

            # Фильтруем None и собираем результаты
            tracks = [t for t in results if t is not None]

            if not tracks:
                print("❌ Не удалось загрузить информацию ни об одном треке")
                return []

            print(f"✅ Успешно загружено {len(tracks)} из {total} треков")
            if len(tracks) < total:
                print(f"⚠️ {total - len(tracks)} треков пропущено из-за ошибок")

            return tracks

    except Exception as e:
        print(f"❌ Ошибка при извлечении плейлиста: {type(e).__name__}: {e}")
        import traceback
        traceback.print_exc()
        return []


def search_yt_dlp(query: str, max_results: int = 50) -> List[Dict]:
    """Поиск треков через yt-dlp"""
    search_url = f"ytsearch{max_results}:{query}"
    
    ydl_opts = {
        "quiet": False,
        "no_warnings": True,
        "force_ipv4": True,
        "prefer_ipv4": True,
        "extract_flat": True,
        
        "geo_bypass": True,
        "geo_bypass_country": "US",
        
        "nocheckcertificate": True,
        "ignoreerrors": True,
        
        "http_headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        }
    }
    
    # Для Linux добавляем cookies
    if IS_LINUX and COOKIES_PATH.exists():
        ydl_opts["cookiefile"] = str(COOKIES_PATH)

    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(search_url, download=False)
            results = []
            if info and info.get("entries"):
                for e in info["entries"]:
                    if not e:
                        continue
                    results.append({
                        "id": e.get("id"),
                        "title": e.get("title") or "Unknown",
                        "artist": e.get("uploader") or e.get("channel") or "",
                        "duration": e.get("duration") or 0,
                        "url": e.get("webpage_url") or e.get("url"),
                    })
            return results
    except Exception as e:
        print(f"❌ Ошибка поиска: {e}")
        return []


class TrackItem(ListItem):
    def __init__(self, track: Dict):
        super().__init__()
        self.track = track
        title = (track.get("title") or "Unknown")[:55]
        artist = (track.get("artist") or track.get("user", {}).get("username") or "?")[:25]
        duration = format_duration(track.get("duration") or 0)
        self.label = Label(f"[bold magenta]{title}[/]  [cyan]@{artist}[/]  [dim]{duration}[/]")

    def compose(self) -> ComposeResult:
        yield self.label


class Player(App):
    CSS = """
    Screen { background: #000; color: #eee; }
    Input { margin: 1 2; width: 100%; }
    ListView { margin: 1 2; height: 1fr; border: round #444; }
    #status { dock: bottom; height: 3; background: #111; content-align: center middle; }
    #progress_container { height: 3; margin: 0 2; }
    #track_progress_container { height: 2; margin: 0 2; dock: bottom; }
    ProgressBar { height: 1; }
    #progress_label { height: 1; content-align: center middle; color: #888; }
    #track_progress { height: 1; }
    #track_time_label { height: 1; content-align: center middle; color: #0ff; }
    """
    BINDINGS = [
        ("space", "toggle_pause", "Пауза"),
        ("n", "next_track", "След"),
        ("p", "prev_track", "Пред"),
        ("ctrl+h", "toggle_history", "История"),
        ("q", "quit", "Выход"),
    ]

    def __init__(self):
        super().__init__()
        self.queue: List[Dict] = []
        self.current_idx: int = 0
        self.current_track: Optional[Dict] = None
        self.current_file: Optional[str] = None
        self.history: List[Dict] = load_history()
        self.history_mode: bool = False
        self._saved_queue: Optional[List[Dict]] = None
        self.loading: bool = False
        self.update_timer: Optional[asyncio.Task] = None
        self.is_paused: bool = False

    def compose(self) -> ComposeResult:
        yield Header()
        yield Input(placeholder="Введи трек/артиста или вставь ссылку (Enter)", id="inp")
        yield ListView(id="list")
        with Static(id="progress_container"):
            yield ProgressBar(total=100, show_eta=False, id="progress")
            yield Static("", id="progress_label")
        with Static(id="track_progress_container"):
            yield ProgressBar(total=100, show_eta=False, id="track_progress")
            yield Static("0:00/0:00 (-0:00)", id="track_time_label")
        
        # Показываем информацию о системе
        system_info = "🐧 Linux режим" if IS_LINUX else "🪟 Windows режим"
        if IS_LINUX and COOKIES_PATH.exists():
            system_info += " (с cookies)"
        elif IS_LINUX:
            system_info += " (без cookies)"
            
        yield Static(f"🎵 {system_info} | Ctrl+H — история", id="status")
        yield Footer()

    def on_mount(self) -> None:
        ensure_temp_dir()
        cleanup_old_files(max_age_seconds=60 * 60)
        self.query_one(Input).focus()
        self.query_one("#progress_container").display = False
        self.query_one("#track_progress_container").display = False
        
        # Выводим информацию о системе
        print("=" * 50)
        print(f"Система: {platform.system()} {platform.release()}")
        print(f"Python: {platform.python_version()}")
        print(f"Директория: {APP_DIR}")
        if IS_LINUX:
            print("🐧 Linux режим активирован")
            if COOKIES_PATH.exists():
                print(f"✅ Найден cookies.txt: {COOKIES_PATH}")
            else:
                print("⚠ cookies.txt не найден - некоторые функции могут не работать")
        print("=" * 50)
        
        # Запускаем обновление прогресса
        self.update_timer = asyncio.create_task(self._update_track_progress())

    async def _update_track_progress(self) -> None:
        """Обновление прогресс-бара трека каждую секунду"""
        while True:
            try:
                await asyncio.sleep(1)

                if not mixer.music.get_busy() and not self.is_paused:
                    # Трек закончился, переключаемся на следующий
                    if self.queue and self.current_idx < len(self.queue) - 1:
                        self.current_idx += 1
                        await self._play_index(self.current_idx)
                    continue

                if self.current_track and mixer.music.get_busy():
                    try:
                        pos_ms = mixer.music.get_pos()
                        if pos_ms < 0:
                            continue

                        pos_sec = pos_ms / 1000.0
                        duration = self.current_track.get("duration", 0)

                        if duration > 0:
                            progress = min(100, int((pos_sec / duration) * 100))
                            track_progress = self.query_one("#track_progress", ProgressBar)
                            track_progress.update(progress=int(pos_sec), total=int(duration))

                            current_time = format_duration(int(pos_sec))
                            total_time = format_duration(duration)
                            remaining = format_duration(int(duration - pos_sec))

                            time_label = self.query_one("#track_time_label", Static)
                            time_label.update(f"{current_time}/{total_time} (-{remaining})")
                    except Exception:
                        pass
            except Exception:
                pass

    async def action_toggle_pause(self) -> None:
        if mixer.music.get_busy():
            mixer.music.pause()
            self.is_paused = True
            self.query_one("#status", Static).update("⏸ Пауза")
        else:
            mixer.music.unpause()
            self.is_paused = False
            if self.current_track:
                title = self.current_track.get('title', 'Unknown')[:40]
                artist = self.current_track.get('artist', '')
                artist_str = f" - {artist[:20]}" if artist else ""
                duration = format_duration(self.current_track.get('duration', 0))
                self.query_one("#status", Static).update(f"▶ {title}{artist_str} ({duration})")

    async def action_next_track(self) -> None:
        if self.queue and self.current_idx < len(self.queue) - 1:
            self.current_idx += 1
            await self._play_index(self.current_idx)

    async def action_prev_track(self) -> None:
        if self.queue and self.current_idx > 0:
            self.current_idx -= 1
            await self._play_index(self.current_idx)

    async def action_toggle_history(self) -> None:
        lv = self.query_one("#list", ListView)
        status = self.query_one("#status", Static)

        if not self.history_mode:
            self._saved_queue = list(self.queue)
            lv.clear()
            if not self.history:
                status.update("🕘 История пуста")
            else:
                for h in self.history:
                    lv.append(ListItem(Label(f"[magenta]{h.get('title')}[/]  [cyan]@{h.get('artist')}[/]")))
                status.update(f"🕘 История ({len(self.history)}) — Enter чтобы воспроизвести")
            self.history_mode = True
        else:
            lv.clear()
            if self._saved_queue:
                self.queue = self._saved_queue
                for t in self.queue:
                    lv.append(TrackItem(t))
                status.update("▶ Вернулись к очереди")
            else:
                status.update("▶ Нет сохранённой очереди")
            self.history_mode = False

    def on_input_submitted(self, event: Input.Submitted) -> None:
        if self.history_mode:
            lv = self.query_one("#list", ListView)
            sel = lv.index or 0
            try:
                hist = self.history[sel]
            except Exception:
                self.query_one("#status", Static).update("❌ Неправильный выбор")
                return
            item = {
                "id": hist.get("id"),
                "title": hist.get("title"),
                "artist": hist.get("artist"),
                "url": hist.get("url"),
            }
            asyncio.create_task(self._play_direct(item))
            return

        q = event.value.strip()
        if not q:
            return
        event.input.value = ""
        asyncio.create_task(self.handle_input(q))

    async def handle_input(self, q: str) -> None:
        lv = self.query_one("#list", ListView)
        status = self.query_one("#status", Static)
        progress_bar = self.query_one("#progress", ProgressBar)
        progress_label = self.query_one("#progress_label", Static)
        progress_container = self.query_one("#progress_container")

        lv.clear()
        status.update("⏳ Загружаю...")

        # Если это ссылка на SoundCloud
        if "soundcloud.com" in q or "snd.sc" in q:
            self.loading = True
            progress_container.display = True
            progress_bar.update(total=100, progress=0)
            progress_label.update("Получаю список треков...")

            def progress_callback(current, total):
                """Обновление прогресс-бара"""
                try:
                    percent = int((current / total) * 100)
                    progress_bar.update(progress=current, total=total)
                    progress_label.update(f"Загружено {current}/{total} треков ({percent}%)")
                except Exception:
                    pass

            tracks = await asyncio.to_thread(simple_playlist_extract, q, progress_callback)

            progress_container.display = False
            self.loading = False

            if not tracks:
                status.update("❌ Не получилось. Попробуй другую ссылку или поиск")
                return

            self.queue = tracks
            self.current_idx = 0
            for t in tracks:
                lv.append(TrackItem(t))
                append_history_item(t)

            total_duration = sum(t.get("duration", 0) for t in tracks)
            status.update(f"✅ {len(tracks)} треков | {format_duration(total_duration)} общее время")
            await self._play_index(0)
        else:
            # Поиск
            tracks = await asyncio.to_thread(search_yt_dlp, q, 50)
            if not tracks:
                status.update("❌ Ничего не найдено")
                return

            self.queue = tracks
            self.current_idx = 0
            for t in tracks:
                lv.append(TrackItem(t))
                append_history_item(t)

            status.update(f"✅ Найдено {len(tracks)}. Играю...")
            await self._play_index(0)

    async def _play_direct(self, item: Dict) -> None:
        status = self.query_one("#status", Static)
        track_progress_container = self.query_one("#track_progress_container")

        self.current_track = item
        self.is_paused = False

        title = item.get('title', 'Unknown')[:40]
        duration = format_duration(item.get('duration', 0))
        status.update(f"⏳ {title}... ({duration})")

        try:
            filename = await asyncio.to_thread(download_track_file, item.get("url"))
        except Exception as e:
            status.update(f"❌ Ошибка: {str(e)[:50]}")
            track_progress_container.display = False
            return

        if self.current_file and os.path.exists(self.current_file):
            try:
                os.remove(self.current_file)
            except Exception:
                pass
            self.current_file = None

        self.current_file = filename
        try:
            mixer.music.load(filename)
            mixer.music.play()
            append_history_item(item)

            # Показываем прогресс-бар трека
            track_progress_container.display = True
            track_progress = self.query_one("#track_progress", ProgressBar)
            track_progress.update(progress=0, total=int(item.get('duration', 100)))

            artist = item.get('artist', '')
            artist_str = f" - {artist[:20]}" if artist else ""
            status.update(f"▶ {title}{artist_str} ({duration})")
        except Exception as e:
            status.update(f"❌ {str(e)[:50]}")
            track_progress_container.display = False

    async def _play_index(self, idx: int) -> None:
        status = self.query_one("#status", Static)
        track_progress_container = self.query_one("#track_progress_container")

        if not (0 <= idx < len(self.queue)):
            return

        track = self.queue[idx]
        self.current_idx = idx
        self.current_track = track
        self.is_paused = False

        title = track.get('title', 'Unknown')[:40]
        duration = format_duration(track.get('duration', 0))
        status.update(f"⏳ {title}... ({duration})")

        if self.current_file and os.path.exists(self.current_file):
            try:
                os.remove(self.current_file)
            except Exception:
                pass
            self.current_file = None

        cleanup_old_files(max_age_seconds=60 * 30)

        try:
            filename = await asyncio.to_thread(download_track_file, track.get("url"))
        except Exception as e:
            status.update(f"❌ Ошибка: {str(e)[:50]}")
            track_progress_container.display = False
            return

        self.current_file = filename
        append_history_item(track)

        try:
            mixer.music.load(filename)
            mixer.music.play()

            # Показываем прогресс-бар трека
            track_progress_container.display = True
            track_progress = self.query_one("#track_progress", ProgressBar)
            track_progress.update(progress=0, total=int(track.get('duration', 100)))

            # Показываем название с длительностью
            artist = track.get('artist', '')
            artist_str = f" - {artist[:20]}" if artist else ""
            status.update(f"▶ {title}{artist_str} ({duration})")
        except Exception as e:
            status.update(f"❌ {str(e)[:50]}")
            track_progress_container.display = False

    def on_list_view_selected(self, event) -> None:
        if self.history_mode:
            return
        item = event.item
        for i, t in enumerate(self.queue):
            if t.get("id") == item.track.get("id") or t.get("url") == item.track.get("url"):
                asyncio.create_task(self._play_index(i))
                return

    async def on_unmount(self) -> None:
        # Останавливаем таймер обновления
        if self.update_timer:
            self.update_timer.cancel()

        try:
            mixer.music.stop()
        except Exception:
            pass
        await asyncio.to_thread(cleanup_temp_dir)


if __name__ == "__main__":
    try:
        print("🚀 Запуск TUI-плеера для SoundCloud...")
        print("=" * 50)
        
        # Проверяем yt-dlp
        try:
            import yt_dlp
            print(f"✅ yt-dlp версия: {yt_dlp.version.__version__}")
        except:
            print("❌ yt-dlp не найден")
            print("Установите: pip install -U yt-dlp")
            
        # Для Linux даем дополнительные инструкции
        if IS_LINUX:
            print("\n🐧 Инструкция для Linux:")
            print("1. Обновите yt-dlp: pip install -U yt-dlp")
            print("2. Для лучшей работы создайте cookies.txt:")
            print("   - Установите расширение для браузера (cookies.txt)")
            print("   - Экспортируйте cookies с soundcloud.com")
            print("   - Положите файл cookies.txt в папку с программой")
            print("3. При проблемах попробуйте запустить с VPN")
            print("=" * 50)
        
        Player().run()
    except KeyboardInterrupt:
        try:
            cleanup_temp_dir()
        finally:
            print("\n👋 Выход...")
