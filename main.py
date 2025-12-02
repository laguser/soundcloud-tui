from typing import List, Dict, Optional, Tuple
import platform
import asyncio
import subprocess
import sys
import os
import tempfile
import shutil
import time
import json
import hashlib
from concurrent.futures import ThreadPoolExecutor, ProcessPoolExecutor, as_completed
from pathlib import Path
from dataclasses import dataclass
import aiohttp
import aiofiles
from collections import deque
import heapq

# Автоустановка зависимостей
REQUIRED = ["textual", "yt_dlp", "requests", "aiohttp"]

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
    print("pip install textual pygame-ce yt-dlp requests aiohttp aiofiles")
    sys.exit(1)

import yt_dlp
import requests
from textual.app import App, ComposeResult
from textual.widgets import Header, Footer, Input, ListView, ListItem, Label, Static, ProgressBar

try:
    import pygame.mixer as mixer
except Exception:
    from pygame import mixer

# Ускоренная инициализация pygame
mixer.pre_init(frequency=48000, size=-16, channels=2, buffer=2048)  # Меньший буфер для быстрого старта
mixer.init()

APP_DIR = Path.cwd()
HISTORY_FILE = APP_DIR / "history.json"
CACHE_DIR = APP_DIR / ".cache"
IS_LINUX = platform.system().lower() == "linux"
COOKIES_PATH = APP_DIR / "cookies.txt"

# Создаем кэш-директорию
CACHE_DIR.mkdir(exist_ok=True)
CACHE_INFO_FILE = CACHE_DIR / "cache_info.json"

@dataclass
class CacheInfo:
    url: str
    filepath: str
    size: int
    timestamp: float
    access_count: int = 0
    
    def to_dict(self):
        return {
            'url': self.url,
            'filepath': self.filepath,
            'size': self.size,
            'timestamp': self.timestamp,
            'access_count': self.access_count
        }
    
    @classmethod
    def from_dict(cls, data):
        return cls(
            url=data['url'],
            filepath=data['filepath'],
            size=data['size'],
            timestamp=data['timestamp'],
            access_count=data.get('access_count', 0)
        )

class AudioCache:
    """Умный кэш для аудиофайлов"""
    def __init__(self, max_size_mb: int = 1024):  # 1GB кэша
        self.max_size = max_size_mb * 1024 * 1024
        self.cache_dir = CACHE_DIR
        self.cache_info = {}
        self.load_cache_info()
        
        # LRU кэш для быстрого доступа
        self.lru = deque()
        self.lru_set = set()
        
    def load_cache_info(self):
        """Загружаем информацию о кэше"""
        try:
            if CACHE_INFO_FILE.exists():
                with open(CACHE_INFO_FILE, 'r') as f:
                    data = json.load(f)
                    for key, item_data in data.items():
                        self.cache_info[key] = CacheInfo.from_dict(item_data)
        except Exception:
            pass
            
    def save_cache_info(self):
        """Сохраняем информацию о кэше"""
        try:
            data = {k: v.to_dict() for k, v in self.cache_info.items()}
            with open(CACHE_INFO_FILE, 'w') as f:
                json.dump(data, f, indent=2)
        except Exception:
            pass
            
    def get_cache_key(self, url: str) -> str:
        """Получаем ключ кэша по URL"""
        return hashlib.md5(url.encode()).hexdigest()
    
    def get_cached_file(self, url: str) -> Optional[str]:
        """Получаем кэшированный файл"""
        key = self.get_cache_key(url)
        if key in self.cache_info:
            info = self.cache_info[key]
            filepath = Path(info.filepath)
            
            if filepath.exists():
                # Обновляем LRU
                if key in self.lru_set:
                    self.lru.remove(key)
                self.lru.appendleft(key)
                self.lru_set.add(key)
                
                # Увеличиваем счетчик обращений
                info.access_count += 1
                info.timestamp = time.time()
                return str(filepath)
            else:
                # Файл удален, чистим запись
                del self.cache_info[key]
                self.save_cache_info()
                
        return None
        
    def add_to_cache(self, url: str, filepath: str):
        """Добавляем файл в кэш"""
        try:
            filepath_obj = Path(filepath)
            if not filepath_obj.exists():
                return
                
            key = self.get_cache_key(url)
            size = filepath_obj.stat().st_size
            
            # Проверяем, не переполнен ли кэш
            self._cleanup_cache(size)
            
            # Добавляем в кэш
            self.cache_info[key] = CacheInfo(
                url=url,
                filepath=str(filepath_obj),
                size=size,
                timestamp=time.time(),
                access_count=1
            )
            
            # Обновляем LRU
            if key in self.lru_set:
                self.lru.remove(key)
            self.lru.appendleft(key)
            self.lru_set.add(key)
            
            self.save_cache_info()
            
        except Exception as e:
            print(f"Ошибка добавления в кэш: {e}")
            
    def _cleanup_cache(self, required_size: int = 0):
        """Очистка кэша по алгоритму LRU"""
        current_size = sum(info.size for info in self.cache_info.values())
        
        # Сортируем файлы по времени последнего доступа
        lru_items = []
        for key, info in self.cache_info.items():
            # Используем комбинацию частоты обращений и времени
            score = info.timestamp / (info.access_count + 1)
            heapq.heappush(lru_items, (score, key, info))
        
        # Удаляем самые старые/редко используемые файлы
        while lru_items and (current_size + required_size > self.max_size):
            _, key, info = heapq.heappop(lru_items)
            
            try:
                if Path(info.filepath).exists():
                    Path(info.filepath).unlink()
                current_size -= info.size
                
                # Удаляем из структур данных
                if key in self.cache_info:
                    del self.cache_info[key]
                if key in self.lru_set:
                    self.lru_set.remove(key)
                    try:
                        self.lru.remove(key)
                    except ValueError:
                        pass
                        
            except Exception:
                pass
        
        self.save_cache_info()

# Глобальный кэш
audio_cache = AudioCache(max_size_mb=1024)  # 1GB кэша

def format_duration(seconds) -> str:
    try:
        m, s = divmod(int(seconds or 0), 60)
        h, m = divmod(m, 60)
        if h > 0:
            return f"{h}:{m:02d}:{s:02d}"
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
    hist = hist[:1000]  # Увеличил историю
    save_history(hist)


def has_ffmpeg() -> bool:
    from shutil import which
    return which("ffmpeg") is not None


async def get_stream_url(url: str) -> Optional[Dict]:
    """Получаем прямую ссылку на поток без скачивания файла"""
    ydl_opts = {
        'quiet': True,
        'no_warnings': True,
        'extract_flat': False,
        'skip_download': True,
        'force_generic_extractor': False,
        
        # Форсируем получение аудио формата
        'format': 'bestaudio/best',
        'prefer_free_formats': True,
        
        # Настройки для SoundCloud
        'extractor_args': {
            'soundcloud': {
                'client_id': ['iZIs9mchVcX5lhVRyQGGAYlNPVldzAoX',
                             'LvWovRaJZlqN2qFgVUeJXzKwd8g209lA']
            }
        },
        
        'http_headers': {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            'Accept': '*/*',
            'Accept-Language': 'en-US,en;q=0.9',
            'Referer': 'https://soundcloud.com/',
        },
        
        'socket_timeout': 10,
        'retries': 3,
    }
    
    if IS_LINUX and COOKIES_PATH.exists():
        ydl_opts['cookiefile'] = str(COOKIES_PATH)
    
    try:
        loop = asyncio.get_event_loop()
        result = await loop.run_in_executor(
            None, 
            lambda: get_stream_url_sync(url, ydl_opts)
        )
        return result
    except Exception as e:
        print(f"Ошибка получения stream URL: {e}")
        return None


def get_stream_url_sync(url: str, ydl_opts: dict) -> Optional[Dict]:
    """Синхронная версия получения stream URL"""
    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(url, download=False)
            
            if not info:
                return None
                
            # Получаем лучший аудио формат
            formats = info.get('formats', [])
            audio_formats = [f for f in formats if f.get('acodec') != 'none']
            
            if audio_formats:
                # Выбираем формат с самым высоким битрейтом
                best_format = max(audio_formats, 
                                key=lambda x: x.get('abr', 0) or x.get('tbr', 0))
                
                stream_url = best_format.get('url')
                if stream_url:
                    return {
                        'stream_url': stream_url,
                        'title': info.get('title', 'Unknown'),
                        'artist': info.get('uploader', ''),
                        'duration': info.get('duration', 0),
                        'url': info.get('webpage_url', url),
                        'id': info.get('id'),
                    }
                    
            # Если не нашли прямой stream, возвращаем информацию для скачивания
            return {
                'stream_url': None,
                'title': info.get('title', 'Unknown'),
                'artist': info.get('uploader', ''),
                'duration': info.get('duration', 0),
                'url': info.get('webpage_url', url),
                'id': info.get('id'),
                'info': info
            }
            
    except Exception as e:
        print(f"Ошибка в get_stream_url_sync: {e}")
        return None


async def download_track_fast(url: str, use_cache: bool = True) -> Optional[str]:
    """Быстрое скачивание трека с кэшированием"""
    # Проверяем кэш
    if use_cache:
        cached = audio_cache.get_cached_file(url)
        if cached:
            print(f"⚡ Использую кэшированный файл: {Path(cached).name}")
            return cached
    
    # Пытаемся получить прямую ссылку на поток
    stream_info = await get_stream_url(url)
    if not stream_info:
        return None
        
    stream_url = stream_info.get('stream_url')
    
    if stream_url:
        # Скачиваем поток напрямую
        try:
            return await download_stream(stream_url, url, stream_info)
        except Exception as e:
            print(f"Ошибка скачивания потока: {e}")
            # Пробуем традиционный метод
    
    # Традиционный метод через yt-dlp
    return await download_via_ytdlp(url, stream_info)


async def download_stream(stream_url: str, original_url: str, info: Dict) -> str:
    """Скачиваем поток напрямую"""
    cache_key = audio_cache.get_cache_key(original_url)
    cache_file = CACHE_DIR / f"{cache_key}.mp3"
    
    # Если файл уже есть в кэше
    if cache_file.exists():
        return str(cache_file)
    
    # Скачиваем поток
    print(f"⬇️ Скачиваю поток: {info.get('title', 'Unknown')[:30]}...")
    
    try:
        async with aiohttp.ClientSession() as session:
            async with session.get(stream_url, timeout=30) as response:
                if response.status == 200:
                    total_size = int(response.headers.get('content-length', 0))
                    
                    # Пишем в файл
                    async with aiofiles.open(cache_file, 'wb') as f:
                        chunk_size = 8192 * 4  # Увеличил размер чанка
                        downloaded = 0
                        
                        async for chunk in response.content.iter_chunked(chunk_size):
                            await f.write(chunk)
                            downloaded += len(chunk)
                            
                            # Прогресс (опционально)
                            if total_size > 0:
                                percent = (downloaded / total_size) * 100
                                if int(percent) % 10 == 0:  # Обновляем каждые 10%
                                    print(f"  📥 {percent:.1f}%")
                    
                    # Добавляем в кэш
                    audio_cache.add_to_cache(original_url, str(cache_file))
                    return str(cache_file)
                    
    except Exception as e:
        print(f"Ошибка скачивания потока: {e}")
        raise


async def download_via_ytdlp(url: str, info: Dict) -> str:
    """Скачивание через yt-dlp (fallback)"""
    print(f"⬇️ Скачиваю через yt-dlp: {info.get('title', 'Unknown')[:30]}...")
    
    cache_key = audio_cache.get_cache_key(url)
    cache_file = CACHE_DIR / f"{cache_key}.mp3"
    
    # Если уже скачано
    if cache_file.exists():
        return str(cache_file)
    
    ydl_opts = {
        'format': 'bestaudio/best',
        'outtmpl': str(CACHE_DIR / f"{cache_key}.%(ext)s"),
        'quiet': False,
        'no_warnings': True,
        
        'postprocessors': [{
            'key': 'FFmpegExtractAudio',
            'preferredcodec': 'mp3',
            'preferredquality': '192',
        }],
        
        'http_headers': {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
        },
        
        'retries': 5,
        'fragment_retries': 5,
        'skip_unavailable_fragments': True,
    }
    
    if IS_LINUX and COOKIES_PATH.exists():
        ydl_opts['cookiefile'] = str(COOKIES_PATH)
    
    try:
        loop = asyncio.get_event_loop()
        result_file = await loop.run_in_executor(
            None,
            lambda: download_via_ytdlp_sync(url, ydl_opts, cache_key)
        )
        
        if result_file:
            audio_cache.add_to_cache(url, result_file)
            return result_file
            
    except Exception as e:
        print(f"Ошибка yt-dlp: {e}")
    
    return None


def download_via_ytdlp_sync(url: str, ydl_opts: dict, cache_key: str) -> Optional[str]:
    """Синхронное скачивание через yt-dlp"""
    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            # Скачиваем
            ydl.download([url])
            
            # Ищем скачанный файл
            for ext in ['mp3', 'm4a', 'webm', 'opus']:
                possible_file = CACHE_DIR / f"{cache_key}.{ext}"
                if possible_file.exists():
                    return str(possible_file)
                    
            # Альтернативный поиск
            for file in CACHE_DIR.glob(f"{cache_key}.*"):
                return str(file)
                
    except Exception as e:
        print(f"Ошибка в download_via_ytdlp_sync: {e}")
    
    return None


class PrefetchManager:
    """Менеджер предзагрузки следующих треков"""
    def __init__(self, prefetch_count: int = 2):
        self.prefetch_count = prefetch_count
        self.prefetch_queue = asyncio.Queue()
        self.prefetch_tasks = []
        self.current_prefetching = set()
        
    async def prefetch_tracks(self, tracks: List[Dict], current_idx: int):
        """Предзагружаем следующие треки"""
        # Очищаем старые задачи
        for task in self.prefetch_tasks:
            task.cancel()
        self.prefetch_tasks.clear()
        self.current_prefetching.clear()
        
        # Определяем треки для предзагрузки
        start_idx = current_idx + 1
        end_idx = min(start_idx + self.prefetch_count, len(tracks))
        
        for idx in range(start_idx, end_idx):
            if idx < len(tracks):
                track = tracks[idx]
                track_url = track.get('url')
                
                if track_url and track_url not in self.current_prefetching:
                    self.current_prefetching.add(track_url)
                    
                    # Запускаем предзагрузку в фоне
                    task = asyncio.create_task(
                        self._prefetch_track(track_url)
                    )
                    self.prefetch_tasks.append(task)
    
    async def _prefetch_track(self, url: str):
        """Предзагрузка одного трека"""
        try:
            # Проверяем кэш
            cached = audio_cache.get_cached_file(url)
            if cached:
                return cached
            
            # Если нет в кэше, скачиваем
            await download_track_fast(url, use_cache=True)
            
        except Exception as e:
            print(f"Ошибка предзагрузки: {e}")
    
    def stop(self):
        """Останавливаем предзагрузку"""
        for task in self.prefetch_tasks:
            task.cancel()
        self.prefetch_tasks.clear()
        self.current_prefetching.clear()


async def get_track_info_fast(url: str) -> Optional[Dict]:
    """Быстрое получение информации о треке"""
    # Пробуем получить информацию через stream метод
    stream_info = await get_stream_url(url)
    if stream_info and stream_info.get('title'):
        return stream_info
    
    # Fallback через yt-dlp
    ydl_opts = {
        'quiet': True,
        'skip_download': True,
        'no_warnings': True,
        'extract_flat': True,
        
        'extractor_args': {
            'soundcloud': {
                'client_id': 'iZIs9mchVcX5lhVRyQGGAYlNPVldzAoX'
            }
        },
        
        'http_headers': {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
        },
        
        'socket_timeout': 5,
    }
    
    if IS_LINUX and COOKIES_PATH.exists():
        ydl_opts['cookiefile'] = str(COOKIES_PATH)
    
    try:
        loop = asyncio.get_event_loop()
        info = await loop.run_in_executor(
            None,
            lambda: get_track_info_sync(url, ydl_opts)
        )
        return info
    except Exception as e:
        print(f"Ошибка получения информации: {e}")
        return None


def get_track_info_sync(url: str, ydl_opts: dict) -> Optional[Dict]:
    """Синхронное получение информации"""
    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(url, download=False)
            if info:
                return {
                    'id': info.get('id'),
                    'title': info.get('title', 'Unknown'),
                    'artist': info.get('uploader', ''),
                    'duration': info.get('duration', 0),
                    'url': info.get('webpage_url', url),
                }
    except Exception:
        pass
    return None


async def search_tracks_fast(query: str, max_results: int = 30) -> List[Dict]:
    """Быстрый поиск треков"""
    search_url = f"ytsearch{max_results}:{query}"
    
    ydl_opts = {
        'quiet': True,
        'extract_flat': True,
        'no_warnings': True,
        'force_ipv4': True,
        
        'extractor_args': {
            'youtube': {
                'flat_playlist': True
            }
        },
        
        'http_headers': {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
        },
        
        'socket_timeout': 5,
    }
    
    try:
        loop = asyncio.get_event_loop()
        tracks = await loop.run_in_executor(
            None,
            lambda: search_tracks_sync(search_url, ydl_opts)
        )
        return tracks[:max_results]  # Ограничиваем количество
    except Exception as e:
        print(f"Ошибка поиска: {e}")
        return []


def search_tracks_sync(search_url: str, ydl_opts: dict) -> List[Dict]:
    """Синхронный поиск"""
    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(search_url, download=False)
            if info and info.get('entries'):
                return [
                    {
                        'id': e.get('id'),
                        'title': e.get('title', 'Unknown'),
                        'artist': e.get('uploader', ''),
                        'duration': e.get('duration', 0),
                        'url': e.get('url') or e.get('webpage_url'),
                    }
                    for e in info['entries']
                    if e
                ]
    except Exception:
        pass
    return []


class TrackItem(ListItem):
    def __init__(self, track: Dict):
        super().__init__()
        self.track = track
        title = (track.get("title") or "Unknown")[:50]
        artist = (track.get("artist") or "?")[:20]
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
    #cache_info { dock: top; height: 1; color: #0f0; text-style: italic; padding: 0 2; }
    """
    BINDINGS = [
        ("space", "toggle_pause", "Пауза"),
        ("n", "next_track", "След"),
        ("p", "prev_track", "Пред"),
        ("ctrl+h", "toggle_history", "История"),
        ("ctrl+c", "clear_cache", "Очистить кэш"),
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
        
        # Менеджер предзагрузки
        self.prefetch_manager = PrefetchManager(prefetch_count=3)
        
        # Кэш сессии для быстрого переключения
        self.session_cache = {}
        
        # Статистика
        self.stats = {
            'cache_hits': 0,
            'cache_misses': 0,
            'stream_downloads': 0,
            'total_downloads': 0
        }

    def compose(self) -> ComposeResult:
        yield Header()
        yield Input(placeholder="Введи трек/артиста или вставь ссылку (Enter)", id="inp")
        yield Static(f"⚡ Кэш: {len(audio_cache.cache_info)} треков", id="cache_info")
        yield ListView(id="list")
        with Static(id="progress_container"):
            yield ProgressBar(total=100, show_eta=False, id="progress")
            yield Static("", id="progress_label")
        with Static(id="track_progress_container"):
            yield ProgressBar(total=100, show_eta=False, id="track_progress")
            yield Static("0:00/0:00", id="track_time_label")
        
        system_info = "🐧 Linux" if IS_LINUX else "🪟 Windows"
        if IS_LINUX and COOKIES_PATH.exists():
            system_info += " (с cookies)"
        elif IS_LINUX:
            system_info += " (без cookies)"
            
        yield Static(f"🎵 {system_info} | ⚡ Быстрый режим | Ctrl+H — история | Ctrl+C — очистить кэш", id="status")
        yield Footer()

    def on_mount(self) -> None:
        self.query_one(Input).focus()
        self.query_one("#progress_container").display = False
        self.query_one("#track_progress_container").display = False
        
        # Выводим информацию о кэше
        cache_size = sum(info.size for info in audio_cache.cache_info.values())
        cache_size_mb = cache_size / (1024 * 1024)
        print(f"⚡ Быстрый режим активирован")
        print(f"📦 Кэш: {len(audio_cache.cache_info)} треков ({cache_size_mb:.1f} MB)")
        
        # Запускаем обновление прогресса
        self.update_timer = asyncio.create_task(self._update_track_progress())
        
        # Запускаем обновление информации о кэше
        asyncio.create_task(self._update_cache_info())

    async def _update_cache_info(self):
        """Обновляем информацию о кэше"""
        while True:
            try:
                cache_size = sum(info.size for info in audio_cache.cache_info.values())
                cache_size_mb = cache_size / (1024 * 1024)
                cache_info = self.query_one("#cache_info", Static)
                hit_rate = 0
                if self.stats['total_downloads'] > 0:
                    hit_rate = (self.stats['cache_hits'] / self.stats['total_downloads']) * 100
                
                cache_info.update(
                    f"⚡ Кэш: {len(audio_cache.cache_info)} треков ({cache_size_mb:.1f} MB) | "
                    f"Хитрейт: {hit_rate:.1f}% | "
                    f"⏱️ Быстрая загрузка"
                )
            except Exception:
                pass
            await asyncio.sleep(2)

    async def _update_track_progress(self) -> None:
        """Обновление прогресс-бара трека"""
        while True:
            try:
                await asyncio.sleep(0.5)  # Чаще обновляем

                if not mixer.music.get_busy() and not self.is_paused:
                    if self.queue and self.current_idx < len(self.queue) - 1:
                        # Автопереход к следующему треку
                        self.current_idx += 1
                        await self._play_index(self.current_idx)
                    continue

                if self.current_track and (mixer.music.get_busy() or self.is_paused):
                    try:
                        pos_ms = mixer.music.get_pos()
                        if pos_ms < 0:
                            continue

                        pos_sec = pos_ms / 1000.0
                        duration = self.current_track.get("duration", 0)

                        if duration > 0:
                            # Обновляем прогресс-бар
                            track_progress = self.query_one("#track_progress", ProgressBar)
                            track_progress.update(progress=int(pos_sec), total=int(duration))

                            current_time = format_duration(int(pos_sec))
                            total_time = format_duration(duration)

                            time_label = self.query_one("#track_time_label", Static)
                            time_label.update(f"{current_time}/{total_time}")
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
                self.query_one("#status", Static).update(f"▶ {title}{artist_str}")

    async def action_next_track(self) -> None:
        if self.queue and self.current_idx < len(self.queue) - 1:
            self.current_idx += 1
            await self._play_index(self.current_idx)

    async def action_prev_track(self) -> None:
        if self.queue and self.current_idx > 0:
            self.current_idx -= 1
            await self._play_index(self.current_idx)

    async def action_clear_cache(self) -> None:
        """Очистка кэша"""
        try:
            # Очищаем файлы кэша
            for file in CACHE_DIR.glob("*"):
                try:
                    file.unlink()
                except Exception:
                    pass
            
            # Очищаем информацию о кэше
            audio_cache.cache_info.clear()
            audio_cache.save_cache_info()
            
            self.query_one("#status", Static).update("✅ Кэш очищен")
            await asyncio.sleep(2)
            
            # Обновляем информацию
            if self.current_track:
                title = self.current_track.get('title', 'Unknown')[:40]
                self.query_one("#status", Static).update(f"▶ {title}")
                
        except Exception as e:
            self.query_one("#status", Static).update(f"❌ Ошибка очистки кэша: {e}")

    async def action_toggle_history(self) -> None:
        lv = self.query_one("#list", ListView)
        status = self.query_one("#status", Static)

        if not self.history_mode:
            self._saved_queue = list(self.queue)
            lv.clear()
            if not self.history:
                status.update("🕘 История пуста")
            else:
                for h in self.history[:50]:  # Показываем только 50 последних
                    lv.append(ListItem(Label(f"[magenta]{h.get('title')}[/]  [cyan]@{h.get('artist')}[/]")))
                status.update(f"🕘 История ({len(self.history[:50])}) — Enter чтобы воспроизвести")
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

        lv.clear()
        status.update("⚡ Быстрый поиск...")

        # Если это ссылка
        if "soundcloud.com" in q or "youtu" in q or "snd.sc" in q:
            status.update("⚡ Получаю информацию...")
            
            # Пробуем получить информацию быстро
            track_info = await get_track_info_fast(q)
            if track_info:
                self.queue = [track_info]
                self.current_idx = 0
                lv.append(TrackItem(track_info))
                append_history_item(track_info)
                
                status.update(f"✅ Найден: {track_info.get('title', 'Unknown')[:40]}")
                await self._play_index(0)
            else:
                status.update("❌ Не удалось получить информацию")
                
        else:
            # Поиск по запросу
            status.update("🔍 Ищу треки...")
            tracks = await search_tracks_fast(q, 20)
            
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
        """Прямое воспроизведение из истории"""
        status = self.query_one("#status", Static)
        track_progress_container = self.query_one("#track_progress_container")

        self.current_track = item
        self.is_paused = False

        title = item.get('title', 'Unknown')[:40]
        status.update(f"⚡ {title}...")

        try:
            # Проверяем кэш
            self.stats['total_downloads'] += 1
            cached = audio_cache.get_cached_file(item.get("url"))
            
            if cached:
                self.stats['cache_hits'] += 1
                filename = cached
                print(f"⚡ Кэш хит! Использую: {Path(filename).name}")
            else:
                self.stats['cache_misses'] += 1
                filename = await download_track_fast(item.get("url"), use_cache=True)
            
            if not filename:
                status.update("❌ Не удалось загрузить трек")
                return

            if self.current_file:
                try:
                    # Останавливаем текущее воспроизведение
                    mixer.music.stop()
                except Exception:
                    pass

            self.current_file = filename
            
            # Загружаем и воспроизводим
            mixer.music.load(filename)
            mixer.music.play()
            
            # Добавляем в историю
            append_history_item(item)
            
            # Показываем прогресс-бар
            track_progress_container.display = True
            track_progress = self.query_one("#track_progress", ProgressBar)
            track_progress.update(progress=0, total=int(item.get('duration', 100)))
            
            # Обновляем статус
            artist = item.get('artist', '')
            artist_str = f" - {artist[:20]}" if artist else ""
            status.update(f"▶ {title}{artist_str}")
            
            # Предзагружаем следующие треки (если есть очередь)
            if self.queue:
                await self.prefetch_manager.prefetch_tracks(self.queue, self.current_idx)
                
        except Exception as e:
            status.update(f"❌ Ошибка: {str(e)[:50]}")
            track_progress_container.display = False

    async def _play_index(self, idx: int) -> None:
        """Воспроизведение трека по индексу в очереди"""
        if not (0 <= idx < len(self.queue)):
            return

        track = self.queue[idx]
        self.current_idx = idx
        self.current_track = track
        self.is_paused = False

        status = self.query_one("#status", Static)
        track_progress_container = self.query_one("#track_progress_container")

        title = track.get('title', 'Unknown')[:40]
        status.update(f"⚡ {title}...")

        try:
            # Проверяем кэш
            self.stats['total_downloads'] += 1
            cached = audio_cache.get_cached_file(track.get("url"))
            
            if cached:
                self.stats['cache_hits'] += 1
                filename = cached
                print(f"⚡ Кэш хит для '{title[:30]}'")
            else:
                self.stats['cache_misses'] += 1
                filename = await download_track_fast(track.get("url"), use_cache=True)
            
            if not filename:
                status.update("❌ Не удалось загрузить трек")
                return

            # Останавливаем текущее воспроизведение
            if mixer.music.get_busy():
                mixer.music.stop()
            
            self.current_file = filename
            
            # Загружаем и воспроизводим
            mixer.music.load(filename)
            mixer.music.play()
            
            # Добавляем в историю
            append_history_item(track)
            
            # Показываем прогресс-бар
            track_progress_container.display = True
            track_progress = self.query_one("#track_progress", ProgressBar)
            track_progress.update(progress=0, total=int(track.get('duration', 100)))
            
            # Обновляем статус
            artist = track.get('artist', '')
            artist_str = f" - {artist[:20]}" if artist else ""
            status.update(f"▶ {title}{artist_str}")
            
            # Предзагружаем следующие треки
            await self.prefetch_manager.prefetch_tracks(self.queue, idx)
            
        except Exception as e:
            status.update(f"❌ Ошибка: {str(e)[:50]}")
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
        """Очистка при выходе"""
        # Останавливаем таймеры
        if self.update_timer:
            self.update_timer.cancel()
            
        # Останавливаем предзагрузку
        self.prefetch_manager.stop()
        
        # Останавливаем воспроизведение
        try:
            mixer.music.stop()
        except Exception:
            pass
        
        # Сохраняем статистику
        try:
            stats_file = APP_DIR / "stats.json"
            with open(stats_file, 'w') as f:
                json.dump(self.stats, f, indent=2)
        except Exception:
            pass
        
        print(f"\n📊 Статистика:")
        print(f"  Хитов кэша: {self.stats['cache_hits']}")
        print(f"  Промахов кэша: {self.stats['cache_misses']}")
        print(f"  Всего загрузок: {self.stats['total_downloads']}")
        if self.stats['total_downloads'] > 0:
            hit_rate = (self.stats['cache_hits'] / self.stats['total_downloads']) * 100
            print(f"  Хитрейт: {hit_rate:.1f}%")


if __name__ == "__main__":
    try:
        print("🚀 Запуск БЫСТРОГО TUI-плеера...")
        print("=" * 60)
        print("⚡ Особенности быстрого режима:")
        print("  • Умный кэш (1GB) с LRU алгоритмом")
        print("  • Прямое стриминг-скачивание")
        print("  • Предзагрузка следующих треков")
        print("  • Быстрый поиск с ограничением 20 результатов")
        print("  • Статистика использования кэша")
        print("=" * 60)
        
        if IS_LINUX:
            print("🐧 Для Linux:")
            if COOKIES_PATH.exists():
                print(f"✅ Использую cookies.txt")
            else:
                print("⚠ cookies.txt не найден - некоторые функции могут быть медленнее")
        
        Player().run()
    except KeyboardInterrupt:
        print("\n👋 Выход...")
    except Exception as e:
        print(f"\n❌ Критическая ошибка: {e}")
        import traceback
        traceback.print_exc()
