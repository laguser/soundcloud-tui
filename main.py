from typing import List, Dict, Optional
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
REQUIRED = ["textual", "pygame-ce", "yt-dlp", "requests"]
for pkg in REQUIRED:
    try:
        __import__(pkg.replace("-", "_"))
    except ImportError:
        print(f"Устанавливаю {pkg}...")
        subprocess.check_call([sys.executable, "-m", "pip", "install", pkg])

import yt_dlp
import requests
from textual.app import App, ComposeResult
from textual.widgets import Header, Footer, Input, ListView, ListItem, Label, Static

try:
    import pygame.mixer as mixer
except Exception:
    from pygame_ce import mixer

mixer.init(frequency=44100, size=-16, channels=2, buffer=512)

APP_DIR = Path.cwd()
HISTORY_FILE = APP_DIR / "history.json"

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

def _build_ydl_opts(outdir: str, use_ffmpeg: bool, geo_bypass: bool=True) -> dict:
    """АГРЕССИВНЫЙ режим: всегда обходим geo!"""
    opts = {
        "format": "bestaudio/best",
        "outtmpl": os.path.join(outdir, "%(id)s.%(ext)s"),
        "quiet": True,
        "no_warnings": True,
        "geo_bypass": True,
        "geo_bypass_country": "US",
    }
    
    if use_ffmpeg:
        opts["postprocessors"] = [{
            "key": "FFmpegExtractAudio",
            "preferredcodec": "mp3",
            "preferredquality": "192",
        }]
    
    opts["http_headers"] = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "Accept-Language": "en-us,en;q=0.5",
        "Sec-Fetch-Mode": "navigate",
    }
    
    return opts

def download_track_file(url: str, outdir: Optional[str] = None, geo_bypass: bool=True) -> str:
    if outdir is None:
        outdir = ensure_temp_dir()
    use_ffmpeg = has_ffmpeg()
    ydl_opts = _build_ydl_opts(outdir, use_ffmpeg, geo_bypass=geo_bypass)
    
    with yt_dlp.YoutubeDL(ydl_opts) as ydl:
        info = ydl.extract_info(url, download=True)
        try:
            filename = ydl.prepare_filename(info)
        except Exception:
            filename = None
    
    if filename and os.path.exists(filename):
        return filename
    
    file_id = info.get("id")
    if file_id:
        for fn in os.listdir(outdir):
            if fn.startswith(file_id):
                return os.path.join(outdir, fn)
    
    files = [os.path.join(outdir, f) for f in os.listdir(outdir)]
    if files:
        files.sort(key=lambda p: os.path.getmtime(p), reverse=True)
        return files[0]
    
    raise FileNotFoundError("Не удалось найти скачанный файл")

def extract_info(url: str, extract_flat: bool = False, geo_bypass: bool=True) -> Dict:
    ydl_opts = {
        "quiet": True,
        "no_warnings": True,
        "geo_bypass": geo_bypass,
        "geo_bypass_country": "US",
    }
    
    if extract_flat:
        ydl_opts["extract_flat"] = "in_playlist"
    
    ydl_opts["http_headers"] = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "Accept-Language": "en-us,en;q=0.5",
    }
    
    with yt_dlp.YoutubeDL(ydl_opts) as ydl:
        return ydl.extract_info(url, download=False)

def fetch_full_entry_info(entry_id_or_url: str, geo_bypass: bool=True) -> Optional[Dict]:
    ydl_opts = {
        "quiet": True,
        "no_warnings": True,
        "geo_bypass": geo_bypass,
        "geo_bypass_country": "US",
    }
    
    ydl_opts["http_headers"] = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "Accept-Language": "en-us,en;q=0.5",
    }
    
    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            info = ydl.extract_info(entry_id_or_url, download=False)
            webpage = info.get("webpage_url") or info.get("url")
            if not webpage:
                return None
            return {
                "id": info.get("id"),
                "title": info.get("title") or webpage,
                "artist": info.get("uploader") or "",
                "duration": info.get("duration") or 0,
                "url": webpage,
            }
    except Exception:
        return None

def build_playlist_entries_fast(url: str, max_workers: int = 8) -> List[Dict]:
    """АГРЕССИВНЫЙ обход geo - пробуем ВСЕ способы"""
    results = []
    
    # ПОПЫТКА 1: С geo_bypass сразу
    try:
        print("🌍 Попытка 1: Загрузка с обходом geo...")
        flat = extract_info(url, extract_flat=True, geo_bypass=True)
        
        if flat and isinstance(flat, dict) and flat.get("entries"):
            ids = []
            for e in flat.get("entries", []):
                eid = e.get("id") or e.get("url") or e.get("webpage_url")
                if eid:
                    ids.append(eid)
            
            if ids:
                print(f"📦 Найдено {len(ids)} треков, загружаю параллельно...")
                with ThreadPoolExecutor(max_workers=max_workers) as ex:
                    futures = {ex.submit(fetch_full_entry_info, i, True): i for i in ids}
                    for fut in as_completed(futures):
                        try:
                            res = fut.result()
                            if res:
                                results.append(res)
                        except Exception:
                            pass
                
                if results:
                    id_to_res = {r["id"]: r for r in results if r.get("id")}
                    ordered = []
                    for i in ids:
                        r = id_to_res.get(i)
                        if r:
                            ordered.append(r)
                    if ordered:
                        print(f"✅ Загружено {len(ordered)} треков!")
                        return ordered
                    return results
    except Exception as e:
        print(f"⚠️ Попытка 1 не удалась: {e}")
    
    # ПОПЫТКА 2: Полная загрузка
    try:
        print("🌍 Попытка 2: Полная загрузка с geo_bypass...")
        full_try = extract_info(url, extract_flat=False, geo_bypass=True)
        tracks = []
        
        if full_try and full_try.get("entries"):
            for e in full_try.get("entries"):
                try:
                    webpage = e.get("webpage_url") or e.get("url")
                    if not webpage:
                        fi = fetch_full_entry_info(e.get("id") or e.get("url"), geo_bypass=True)
                        if fi:
                            tracks.append(fi)
                        continue
                    
                    tracks.append({
                        "id": e.get("id"),
                        "title": e.get("title") or webpage,
                        "artist": e.get("uploader") or e.get("artist") or "",
                        "duration": e.get("duration") or 0,
                        "url": webpage,
                    })
                except Exception:
                    continue
            
            if tracks:
                print(f"✅ Загружено {len(tracks)} треков!")
                return tracks
    except Exception as e:
        print(f"⚠️ Попытка 2 не удалась: {e}")
    
    # ПОПЫТКА 3: Без geo (на всякий случай)
    try:
        print("🔄 Попытка 3: Стандартная загрузка...")
        flat = extract_info(url, extract_flat=True, geo_bypass=False)
        
        if flat and isinstance(flat, dict) and flat.get("entries"):
            ids = []
            for e in flat.get("entries", []):
                eid = e.get("id") or e.get("url") or e.get("webpage_url")
                if eid:
                    ids.append(eid)
            
            if ids:
                with ThreadPoolExecutor(max_workers=max_workers) as ex:
                    futures = {ex.submit(fetch_full_entry_info, i, False): i for i in ids}
                    for fut in as_completed(futures):
                        try:
                            res = fut.result()
                            if res:
                                results.append(res)
                        except Exception:
                            pass
                
                if results:
                    print(f"✅ Загружено {len(results)} треков!")
                    return results
    except Exception as e:
        print(f"⚠️ Попытка 3 не удалась: {e}")
    
    print("❌ Все попытки не удались. Возможно нужен VPN.")
    return []

def search_yt_dlp(query: str, max_results: int = 50) -> List[Dict]:
    search_url = f"ytsearch{max_results}:{query}"
    try:
        info = extract_info(search_url, geo_bypass=True)
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
    except Exception:
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

    def compose(self) -> ComposeResult:
        yield Header()
        yield Input(placeholder="Введи трек/артиста или вставь ссылку на трек/плейлист (Enter)", id="inp")
        yield ListView(id="list")
        yield Static("🎵 Готово (с агрессивным обходом geo). Ctrl+H — история", id="status")
        yield Footer()

    def on_mount(self) -> None:
        ensure_temp_dir()
        cleanup_old_files(max_age_seconds=60*60)
        self.query_one(Input).focus()

    async def action_toggle_pause(self) -> None:
        if mixer.music.get_busy():
            mixer.music.pause()
            self.query_one("#status", Static).update("⸸ Пауза")
        else:
            mixer.music.unpause()
            if self.current_track:
                self.query_one("#status", Static).update(f"▶ {self.current_track.get('title')}")

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
                status.update(f"🕘 История ({len(self.history)}) — Enter чтобы воспроизвести, Ctrl+H чтобы вернуться")
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
                self.query_one("#status", Static).update("❌ Неправильный выбор истории")
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
        status.update("⏳ Загрузка...")
        
        if "soundcloud.com" in q or "snd.sc" in q:
            status.update("⏳ Формирую список треков (с обходом geo)...")
            tracks = await asyncio.to_thread(build_playlist_entries_fast, q, 8)
            
            if not tracks:
                status.update("❌ Не удалось загрузить. Попробуй VPN или другую ссылку.")
                return
            
            self.queue = tracks
            self.current_idx = 0
            for t in tracks:
                lv.append(TrackItem(t))
                append_history_item(t)
            status.update(f"📋 Загружено {len(tracks)} треков. Воспроизвожу первый.")
            await self._play_index(0)
        else:
            status.update("⏳ Поиск...")
            tracks = await asyncio.to_thread(search_yt_dlp, q, 50)
            if not tracks:
                status.update("⚠️ Ничего не найдено")
                return
            self.queue = tracks
            self.current_idx = 0
            for t in tracks:
                lv.append(TrackItem(t))
                append_history_item(t)
            status.update(f"🔍 Найдено: {len(tracks)} треков. Воспроизвожу первый.")
            await self._play_index(0)

    async def _play_direct(self, item: Dict) -> None:
        status = self.query_one("#status", Static)
        status.update(f"⏳ Воспроизвожу: {item.get('title')}")
        try:
            filename = await asyncio.to_thread(download_track_file, item.get("url"))
        except Exception as e:
            status.update(f"❌ Ошибка загрузки: {e}")
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
            status.update(f"▶ {item.get('title')}")
        except Exception as e:
            status.update(f"❌ Ошибка воспроизведения: {e}")

    async def _play_index(self, idx: int) -> None:
        status = self.query_one("#status", Static)
        if not (0 <= idx < len(self.queue)):
            return
        
        track = self.queue[idx]
        self.current_idx = idx
        self.current_track = track
        status.update(f"⏳ Подготовка: {track.get('title')}")
        
        if self.current_file and os.path.exists(self.current_file):
            try:
                os.remove(self.current_file)
            except Exception:
                pass
            self.current_file = None
        
        cleanup_old_files(max_age_seconds=60*30)
        
        try:
            filename = await asyncio.to_thread(download_track_file, track.get("url"))
        except Exception as e:
            err = str(e)
            if "geo" in err.lower() or "not available" in err.lower():
                status.update(f"❌ Geo-блок. Попробуй VPN или другой трек.")
            else:
                status.update(f"❌ Ошибка: {e}")
            return
        
        self.current_file = filename
        append_history_item(track)
        
        try:
            mixer.music.load(filename)
            mixer.music.play()
            status.update(f"▶ {track.get('title')}")
        except Exception as e:
            status.update(f"❌ Ошибка воспроизведения: {e}")

    def on_list_view_selected(self, event) -> None:
        if self.history_mode:
            return
        item = event.item
        for i, t in enumerate(self.queue):
            if t.get("id") == item.track.get("id") or t.get("url") == item.track.get("url"):
                asyncio.create_task(self._play_index(i))
                return

    async def on_unmount(self) -> None:
        try:
            mixer.music.stop()
        except Exception:
            pass
        await asyncio.to_thread(cleanup_temp_dir)

if __name__ == "__main__":
    try:
        print("🚀 Запускаю SoundCloud TUI с АГРЕССИВНЫМ обходом geo...")
        Player().run()
    except KeyboardInterrupt:
        try:
            cleanup_temp_dir()
        finally:
            print("Выход...")
