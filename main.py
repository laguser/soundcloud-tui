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

# Директория для кэша
CACHE_DIR = Path.home() / ".cache" / "soundcloud_tui"
CACHE_DIR.mkdir(parents=True, exist_ok=True)

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

def get_cache_file(url: str) -> Path:
    """Получаем путь к кэшированному файлу"""
    import hashlib
    key = hashlib.md5(url.encode()).hexdigest()
    return CACHE_DIR / f"{key}.mp3"

def is_cached(url: str) -> bool:
    """Проверяем, есть ли трек в кэше"""
    cache_file = get_cache_file(url)
    return cache_file.exists()

def get_cached_file(url: str) -> Optional[str]:
    """Получаем кэшированный файл"""
    cache_file = get_cache_file(url)
    if cache_file.exists():
        return str(cache_file)
    return None

def add_to_cache(url: str, filepath: str):
    """Добавляем файл в кэш"""
    try:
        cache_file = get_cache_file(url)
        if Path(filepath).exists() and not cache_file.exists():
            shutil.copy2(filepath, cache_file)
            print(f"✅ Добавлено в кэш: {cache_file.name}")
    except Exception as e:
        print(f"❌ Ошибка кэширования: {e}")

def cleanup_old_cache(max_files: int = 100):
    """Очистка старых файлов кэша"""
    try:
        files = list(CACHE_DIR.glob("*.mp3"))
        if len(files) > max_files:
            # Сортируем по времени модификации (старые сначала)
            files.sort(key=lambda x: x.stat().st_mtime)
            # Удаляем самые старые
            for i in range(len(files) - max_files):
                try:
                    files[i].unlink()
                    print(f"🧹 Удален старый кэш: {files[i].name}")
                except Exception:
                    pass
    except Exception:
        pass

def get_simple_track_info(url: str) -> Optional[Dict]:
    """Простое получение информации о треке"""
    ydl_opts = {
        "quiet": True,
        "skip_download": True,
        "no_warnings": True,
        
        # Критически важные настройки для Linux
        "extractor_args": {
            "soundcloud": {
                "client_id": "iZIs9mchVcX5lhVRyQGGAYlNPVldzAoX"
            }
        },
        
        "http_headers": {
            "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            "Accept": "*/*",
            "Accept-Language": "en-US,en;q=0.9",
            "Referer": "https://soundcloud.com/",
        },
        
        "socket_timeout": 10,
    }
    
    # Если есть cookies - используем
    if COOKIES_PATH.exists():
        ydl_opts["cookiefile"] = str(COOKIES_PATH)
        print("🍪 Использую cookies.txt")
    
    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(url, download=False)
            if info:
                return {
                    "id": info.get("id"),
                    "title": info.get("title") or "Unknown",
                    "artist": info.get("uploader") or info.get("artist") or "",
                    "duration": info.get("duration") or 0,
                    "url": info.get("webpage_url") or url,
                }
    except Exception as e:
        print(f"❌ Ошибка получения информации: {e}")
        # Пробуем альтернативный метод
        return get_track_info_fallback(url)
    
    return None

def get_track_info_fallback(url: str) -> Optional[Dict]:
    """Альтернативный метод получения информации"""
    try:
        # Пробуем через subprocess
        cmd = ["yt-dlp", "--skip-download", "--print-json", "--no-warnings", url]
        
        if COOKIES_PATH.exists():
            cmd.extend(["--cookies", str(COOKIES_PATH)])
        
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=15
        )
        
        if result.returncode == 0 and result.stdout:
            info = json.loads(result.stdout)
            return {
                "id": info.get("id"),
                "title": info.get("title") or "Unknown",
                "artist": info.get("uploader") or info.get("artist") or "",
                "duration": info.get("duration") or 0,
                "url": info.get("webpage_url") or url,
            }
    except Exception as e:
        print(f"❌ Fallback тоже не сработал: {e}")
    
    return None

def download_track_simple(url: str) -> Optional[str]:
    """Простое скачивание трека"""
    print(f"⬇️  Начинаю загрузку...")
    
    # Сначала проверяем кэш
    cached = get_cached_file(url)
    if cached:
        print("⚡ Использую кэшированную версию")
        return cached
    
    # Создаем временную директорию
    temp_dir = tempfile.mkdtemp(prefix="sc_dl_")
    
    ydl_opts = {
        "format": "bestaudio/best",
        "outtmpl": os.path.join(temp_dir, "%(id)s.%(ext)s"),
        "quiet": False,
        "no_warnings": False,
        "verbose": True,  # Включаем verbose для отладки
        
        "extractor_args": {
            "soundcloud": {
                "client_id": "iZIs9mchVcX5lhVRyQGGAYlNPVldzAoX"
            }
        },
        
        "http_headers": {
            "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
            "Accept": "*/*",
            "Accept-Language": "en-US,en;q=0.9",
            "Referer": "https://soundcloud.com/",
        },
        
        "retries": 3,
        "fragment_retries": 3,
        "socket_timeout": 30,
    }
    
    # Добавляем cookies если есть
    if COOKIES_PATH.exists():
        ydl_opts["cookiefile"] = str(COOKIES_PATH)
    
    # Пробуем с конвертацией в mp3
    try:
        # Проверяем есть ли ffmpeg
        ffmpeg_result = subprocess.run(["which", "ffmpeg"], capture_output=True, text=True)
        has_ffmpeg = ffmpeg_result.returncode == 0
        
        if has_ffmpeg:
            ydl_opts["postprocessors"] = [{
                "key": "FFmpegExtractAudio",
                "preferredcodec": "mp3",
                "preferredquality": "192",
            }]
            print("✅ FFmpeg найден, конвертирую в MP3")
        else:
            print("⚠️ FFmpeg не найден, скачиваю исходный формат")
    
    except Exception:
        pass
    
    try:
        print(f"📦 Временная директория: {temp_dir}")
        
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            # Сначала получаем информацию
            print("🔍 Получаю информацию о треке...")
            info = ydl.extract_info(url, download=False)
            
            if not info:
                print("❌ Не удалось получить информацию")
                return None
            
            print(f"🎵 Найден: {info.get('title', 'Unknown')}")
            print(f"🎤 Исполнитель: {info.get('uploader', 'Unknown')}")
            print(f"⏱️ Длительность: {format_duration(info.get('duration', 0))}")
            
            # Теперь скачиваем
            print("⬇️  Скачиваю...")
            ydl.download([url])
            
            # Ищем скачанный файл
            for file in os.listdir(temp_dir):
                if file.endswith(('.mp3', '.m4a', '.webm', '.opus')):
                    filepath = os.path.join(temp_dir, file)
                    print(f"✅ Скачано: {file}")
                    
                    # Копируем в кэш
                    add_to_cache(url, filepath)
                    
                    # Очищаем старый кэш
                    cleanup_old_cache()
                    
                    return filepath
        
        print("❌ Не найден скачанный файл")
        return None
        
    except Exception as e:
        print(f"❌ Ошибка скачивания: {e}")
        import traceback
        traceback.print_exc()
        return None
        
    finally:
        # Не удаляем temp_dir сразу, файл может использоваться
        pass

def search_soundcloud_simple(query: str, limit: int = 20) -> List[Dict]:
    """Простой поиск на SoundCloud"""
    print(f"🔍 Ищу: {query}")
    
    ydl_opts = {
        "quiet": True,
        "no_warnings": True,
        "extract_flat": True,
        
        "extractor_args": {
            "soundcloud": {
                "client_id": "iZIs9mchVcX5lhVRyQGGAYlNPVldzAoX"
            }
        },
        
        "http_headers": {
            "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
        },
        
        "socket_timeout": 10,
    }
    
    if COOKIES_PATH.exists():
        ydl_opts["cookiefile"] = str(COOKIES_PATH)
    
    try:
        search_url = f"ytsearch{limit}:{query}"
        
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(search_url, download=False)
            
            results = []
            if info and info.get("entries"):
                for entry in info["entries"]:
                    if not entry:
                        continue
                    
                    # Проверяем, это SoundCloud или YouTube
                    if "soundcloud.com" in entry.get("url", "") or "soundcloud.com" in entry.get("webpage_url", ""):
                        results.append({
                            "id": entry.get("id"),
                            "title": entry.get("title") or "Unknown",
                            "artist": entry.get("uploader") or entry.get("channel") or "",
                            "duration": entry.get("duration") or 0,
                            "url": entry.get("webpage_url") or entry.get("url"),
                            "source": "soundcloud"
                        })
                    else:
                        # Это YouTube результат
                        results.append({
                            "id": entry.get("id"),
                            "title": entry.get("title") or "Unknown",
                            "artist": entry.get("uploader") or entry.get("channel") or "",
                            "duration": entry.get("duration") or 0,
                            "url": entry.get("webpage_url") or entry.get("url"),
                            "source": "youtube"
                        })
            
            print(f"✅ Найдено результатов: {len(results)}")
            return results[:limit]
            
    except Exception as e:
        print(f"❌ Ошибка поиска: {e}")
        return []

class TrackItem(ListItem):
    def __init__(self, track: Dict):
        super().__init__()
        self.track = track
        title = (track.get("title") or "Unknown")[:50]
        artist = (track.get("artist") or "?")[:25]
        duration = format_duration(track.get("duration") or 0)
        source = "🎵" if track.get("source") == "soundcloud" else "📺"
        self.label = Label(f"{source} [bold magenta]{title}[/]  [cyan]@{artist}[/]  [dim]{duration}[/]")

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
    #cache_info { dock: top; height: 1; color: #0f0; padding: 0 2; }
    """
    
    BINDINGS = [
        ("space", "toggle_pause", "Пауза"),
        ("n", "next_track", "След"),
        ("p", "prev_track", "Пред"),
        ("ctrl+h", "toggle_history", "История"),
        ("ctrl+r", "reload_cache", "Обновить кэш"),
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
        self.is_paused: bool = False
        self.update_task: Optional[asyncio.Task] = None

    def compose(self) -> ComposeResult:
        yield Header()
        yield Input(placeholder="Введи запрос или ссылку на SoundCloud (Enter)", id="inp")
        cache_count = len(list(CACHE_DIR.glob("*.mp3")))
        yield Static(f"📦 Кэш: {cache_count} треков | ⚡ Простой режим", id="cache_info")
        yield ListView(id="list")
        with Static(id="progress_container"):
            yield ProgressBar(total=100, show_eta=False, id="progress")
            yield Static("", id="progress_label")
        with Static(id="track_progress_container"):
            yield ProgressBar(total=100, show_eta=False, id="track_progress")
            yield Static("0:00/0:00", id="track_time_label")
        
        system_msg = "🐧 Linux" if IS_LINUX else "🪟 Windows"
        if IS_LINUX and COOKIES_PATH.exists():
            system_msg += " (с cookies)"
        
        yield Static(f"{system_msg} | Ctrl+H — история | Ctrl+R — обновить кэш", id="status")
        yield Footer()

    def on_mount(self) -> None:
        """При старте приложения"""
        self.query_one(Input).focus()
        self.query_one("#progress_container").display = False
        self.query_one("#track_progress_container").display = False
        
        print("=" * 50)
        print("🎵 SoundCloud TUI Player")
        print("=" * 50)
        print(f"📂 Директория: {APP_DIR}")
        print(f"📦 Кэш: {CACHE_DIR}")
        
        cache_files = list(CACHE_DIR.glob("*.mp3"))
        print(f"📊 В кэше: {len(cache_files)} треков")
        
        if IS_LINUX:
            print("🐧 Linux режим")
            if COOKIES_PATH.exists():
                print("✅ Файл cookies.txt найден")
            else:
                print("⚠️  Файл cookies.txt не найден")
                print("   Для лучшей работы создайте cookies.txt в папке с программой")
        
        print("=" * 50)
        print("\n💡 Как использовать:")
        print("• Введи название трека или артиста для поиска")
        print("• Или вставь прямую ссылку на SoundCloud")
        print("• Enter - поиск/загрузка")
        print("• Стрелки вверх/вниз - навигация")
        print("• Space - пауза/продолжить")
        print("• N/P - следующий/предыдущий трек")
        print("• Ctrl+H - история прослушивания")
        print("• Ctrl+R - обновить информацию о кэше")
        print("• Q - выход")
        print("=" * 50)
        
        # Запускаем обновление прогресса
        self.update_task = asyncio.create_task(self._update_progress())

    async def _update_progress(self):
        """Обновление прогресс-бара"""
        while True:
            try:
                await asyncio.sleep(0.5)
                
                if self.current_track and (mixer.music.get_busy() or self.is_paused):
                    try:
                        pos_ms = mixer.music.get_pos()
                        if pos_ms >= 0:
                            pos_sec = pos_ms / 1000.0
                            duration = self.current_track.get("duration", 0)
                            
                            if duration > 0:
                                # Прогресс-бар трека
                                track_progress = self.query_one("#track_progress", ProgressBar)
                                track_progress.update(progress=int(pos_sec), total=int(duration))
                                
                                # Время
                                current = format_duration(int(pos_sec))
                                total = format_duration(duration)
                                self.query_one("#track_time_label", Static).update(f"{current}/{total}")
                    except Exception:
                        pass
                        
            except Exception:
                pass

    async def action_toggle_pause(self):
        """Пауза/продолжить"""
        if mixer.music.get_busy():
            mixer.music.pause()
            self.is_paused = True
            if self.current_track:
                title = self.current_track.get('title', 'Unknown')[:40]
                self.query_one("#status", Static).update(f"⏸ Пауза: {title}")
        else:
            mixer.music.unpause()
            self.is_paused = False
            if self.current_track:
                title = self.current_track.get('title', 'Unknown')[:40]
                artist = self.current_track.get('artist', '')
                if artist:
                    self.query_one("#status", Static).update(f"▶ {title} - {artist}")
                else:
                    self.query_one("#status", Static).update(f"▶ {title}")

    async def action_next_track(self):
        """Следующий трек"""
        if self.queue and self.current_idx < len(self.queue) - 1:
            self.current_idx += 1
            await self.play_current_track()

    async def action_prev_track(self):
        """Предыдущий трек"""
        if self.queue and self.current_idx > 0:
            self.current_idx -= 1
            await self.play_current_track()

    async def action_reload_cache(self):
        """Обновить информацию о кэше"""
        cache_files = list(CACHE_DIR.glob("*.mp3"))
        cache_info = self.query_one("#cache_info", Static)
        cache_info.update(f"📦 Кэш: {len(cache_files)} треков | ⚡ Обновлено")
        
        status = self.query_one("#status", Static)
        status.update(f"✅ Кэш обновлен: {len(cache_files)} треков")
        await asyncio.sleep(2)
        
        if self.current_track:
            title = self.current_track.get('title', 'Unknown')[:40]
            status.update(f"▶ {title}")

    async def action_toggle_history(self):
        """Переключение в режим истории"""
        lv = self.query_one("#list", ListView)
        status = self.query_one("#status", Static)
        
        if not self.history_mode:
            # Переходим в режим истории
            self._saved_queue = list(self.queue)
            lv.clear()
            
            if not self.history:
                status.update("📜 История пуста")
            else:
                # Показываем последние 50 треков
                for h in self.history[:50]:
                    title = h.get('title', 'Unknown')[:50]
                    artist = h.get('artist', '')[:20]
                    lv.append(ListItem(Label(f"[magenta]{title}[/]  [cyan]@{artist}[/]")))
                
                status.update(f"📜 История ({len(self.history[:50])}) - Enter для воспроизведения")
            
            self.history_mode = True
            
        else:
            # Возвращаемся к очереди
            lv.clear()
            
            if self._saved_queue:
                self.queue = self._saved_queue
                for track in self.queue:
                    lv.append(TrackItem(track))
                status.update("✅ Вернулись к очереди")
            else:
                status.update("📭 Очередь пуста")
            
            self.history_mode = False

    def on_input_submitted(self, event: Input.Submitted):
        """Обработка ввода"""
        query = event.value.strip()
        if not query:
            return
        
        event.input.value = ""
        asyncio.create_task(self.process_input(query))

    async def process_input(self, query: str):
        """Обработка поискового запроса"""
        lv = self.query_one("#list", ListView)
        status = self.query_one("#status", Static)
        
        lv.clear()
        
        # Проверяем режим истории
        if self.history_mode:
            selected_idx = lv.index or 0
            if selected_idx < len(self.history):
                track = self.history[selected_idx]
                await self.play_direct_url(track.get('url'), track)
            return
        
        status.update("🔍 Поиск...")
        
        # Проверяем, это ссылка или поисковый запрос
        if any(domain in query.lower() for domain in ['soundcloud.com', 'snd.sc', 'youtube.com', 'youtu.be']):
            # Это ссылка
            status.update("🌐 Загружаю информацию...")
            await self.handle_url(query)
        else:
            # Это поисковый запрос
            status.update(f"🔍 Ищу: {query[:30]}...")
            await self.handle_search(query)

    async def handle_url(self, url: str):
        """Обработка URL"""
        status = self.query_one("#status", Static)
        
        # Получаем информацию о треке
        loop = asyncio.get_event_loop()
        track_info = await loop.run_in_executor(None, get_simple_track_info, url)
        
        if not track_info:
            status.update("❌ Не удалось получить информацию")
            return
        
        # Добавляем в очередь
        self.queue = [track_info]
        self.current_idx = 0
        
        # Показываем в списке
        lv = self.query_one("#list", ListView)
        lv.append(TrackItem(track_info))
        
        # Начинаем воспроизведение
        await self.play_current_track()

    async def handle_search(self, query: str):
        """Обработка поиска"""
        status = self.query_one("#status", Static)
        lv = self.query_one("#list", ListView)
        
        # Выполняем поиск
        loop = asyncio.get_event_loop()
        results = await loop.run_in_executor(None, search_soundcloud_simple, query, 20)
        
        if not results:
            status.update("❌ Ничего не найдено")
            return
        
        # Обновляем очередь
        self.queue = results
        self.current_idx = 0
        
        # Показываем результаты
        for track in results:
            lv.append(TrackItem(track))
        
        status.update(f"✅ Найдено {len(results)} треков. Выберите трек для воспроизведения")
        
        # Если есть результаты, начинаем воспроизводить первый
        if results:
            await self.play_current_track()

    async def play_direct_url(self, url: str, track_info: Optional[Dict] = None):
        """Воспроизведение по прямой ссылке"""
        status = self.query_one("#status", Static)
        
        if not track_info:
            loop = asyncio.get_event_loop()
            track_info = await loop.run_in_executor(None, get_simple_track_info, url)
            
            if not track_info:
                status.update("❌ Не удалось получить информацию")
                return
        
        # Создаем очередь из одного трека
        self.queue = [track_info]
        self.current_idx = 0
        self.current_track = track_info
        
        title = track_info.get('title', 'Unknown')[:40]
        status.update(f"⬇️  Загружаю: {title}...")
        
        # Скачиваем трек
        loop = asyncio.get_event_loop()
        filepath = await loop.run_in_executor(None, download_track_simple, url)
        
        if not filepath:
            status.update("❌ Ошибка загрузки")
            return
        
        # Воспроизводим
        try:
            if self.current_file:
                try:
                    mixer.music.stop()
                except Exception:
                    pass
            
            self.current_file = filepath
            mixer.music.load(filepath)
            mixer.music.play()
            
            # Добавляем в историю
            append_history_item(track_info)
            
            # Показываем прогресс-бар
            self.query_one("#track_progress_container").display = True
            track_progress = self.query_one("#track_progress", ProgressBar)
            duration = track_info.get('duration', 100)
            track_progress.update(progress=0, total=int(duration))
            
            # Обновляем статус
            artist = track_info.get('artist', '')
            if artist:
                status.update(f"▶ {title} - {artist}")
            else:
                status.update(f"▶ {title}")
                
        except Exception as e:
            status.update(f"❌ Ошибка воспроизведения: {str(e)[:50]}")
            self.query_one("#track_progress_container").display = False

    async def play_current_track(self):
        """Воспроизведение текущего трека из очереди"""
        if not self.queue or self.current_idx >= len(self.queue):
            return
        
        track = self.queue[self.current_idx]
        await self.play_direct_url(track.get('url'), track)

    def on_list_view_selected(self, event):
        """Обработка выбора трека из списка"""
        if self.history_mode:
            # В режиме истории обрабатываем по-другому
            return
        
        item = event.item
        for i, track in enumerate(self.queue):
            if (track.get('id') == item.track.get('id') or 
                track.get('url') == item.track.get('url')):
                self.current_idx = i
                asyncio.create_task(self.play_current_track())
                return

    async def on_unmount(self):
        """Очистка при выходе"""
        if self.update_task:
            self.update_task.cancel()
        
        try:
            mixer.music.stop()
        except Exception:
            pass
        
        # Сохраняем историю
        save_history(self.history)
        
        print("\n👋 Выход из программы")
        print("💾 История сохранена")

if __name__ == "__main__":
    try:
        print("🚀 Запуск SoundCloud TUI Player...")
        
        # Проверяем версию yt-dlp
        try:
            yt_dlp_version = yt_dlp.version.__version__
            print(f"✅ yt-dlp версия: {yt_dlp_version}")
        except:
            print("⚠️  Не удалось определить версию yt-dlp")
        
        # Проверяем cookies для Linux
        if IS_LINUX:
            if COOKIES_PATH.exists():
                print(f"✅ Найден cookies.txt: {COOKIES_PATH}")
            else:
                print("⚠️  Файл cookies.txt не найден")
                print("   Создайте его для лучшей работы с SoundCloud:")
                print("   1. Установите расширение 'cookies.txt' в браузере")
                print("   2. Залогиньтесь на soundcloud.com")
                print("   3. Экспортируйте cookies в файл cookies.txt")
                print("   4. Положите файл в папку с программой")
        
        print("\n" + "=" * 50)
        
        # Запускаем приложение
        app = Player()
        app.run()
        
    except KeyboardInterrupt:
        print("\n\n👋 Выход по Ctrl+C")
    except Exception as e:
        print(f"\n❌ Критическая ошибка: {e}")
        import traceback
        traceback.print_exc()
