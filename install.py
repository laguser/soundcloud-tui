#!/usr/bin/env python3
import os
import sys
import subprocess
import shutil
from pathlib import Path

def install_dependencies():
    """Установка Python зависимостей"""
    libs = ["textual", "pygame-ce", "yt-dlp", "requests"]
    
    print("=" * 60)
    print("📦 Установка зависимостей Python")
    print("=" * 60)
    
    failed = []
    for lib in libs:
        try:
            print(f"Устанавливаю {lib}...")
            subprocess.check_call(
                [sys.executable, "-m", "pip", "install", lib],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE
            )
            print(f"✓ {lib} успешно установлен")
        except subprocess.CalledProcessError:
            print(f"✗ Ошибка при установке {lib}")
            failed.append(lib)
        print()
    
    if failed:
        print("⚠️  Не удалось установить:", ", ".join(failed))
        return False
    return True

def create_launcher_linux():
    """Создание launcher скрипта для Linux"""
    script_dir = Path(__file__).parent.absolute()
    main_py = script_dir / "main.py"
    
    # Создаём исполняемый скрипт
    launcher_content = f"""#!/bin/bash
cd "{script_dir}"
{sys.executable} "{main_py}" "$@"
"""
    
    launcher_path = script_dir / "spotify-tui"
    with open(launcher_path, "w") as f:
        f.write(launcher_content)
    
    # Делаем исполняемым
    os.chmod(launcher_path, 0o755)
    
    return launcher_path

def create_launcher_windows():
    """Создание launcher скрипта для Windows"""
    script_dir = Path(__file__).parent.absolute()
    main_py = script_dir / "main.py"
    
    # Создаём .bat файл
    launcher_content = f"""@echo off
cd /d "{script_dir}"
"{sys.executable}" "{main_py}" %*
"""
    
    launcher_path = script_dir / "spotify-tui.bat"
    with open(launcher_path, "w") as f:
        f.write(launcher_content)
    
    return launcher_path

def add_to_path_linux(local_bin):
    """Автоматическое добавление в PATH для Linux"""
    home = Path.home()
    
    # Определяем shell конфигурационные файлы
    shell_configs = []
    
    # Проверяем какой shell используется
    shell = os.environ.get('SHELL', '')
    
    if 'zsh' in shell:
        shell_configs = [home / '.zshrc', home / '.zprofile']
    elif 'fish' in shell:
        shell_configs = [home / '.config' / 'fish' / 'config.fish']
    else:  # bash по умолчанию
        shell_configs = [home / '.bashrc', home / '.bash_profile', home / '.profile']
    
    path_line = f'export PATH="$HOME/.local/bin:$PATH"'
    path_line_fish = 'set -gx PATH $HOME/.local/bin $PATH'
    
    added = False
    for config_file in shell_configs:
        if config_file.exists():
            # Проверяем, уже есть ли PATH в файле
            with open(config_file, 'r') as f:
                content = f.read()
                if '.local/bin' in content and 'PATH' in content:
                    print(f"✓ PATH уже настроен в {config_file}")
                    return True
            
            # Добавляем PATH
            try:
                with open(config_file, 'a') as f:
                    f.write('\n# Added by spotify-tui installer\n')
                    if 'fish' in str(config_file):
                        f.write(path_line_fish + '\n')
                    else:
                        f.write(path_line + '\n')
                print(f"✓ Добавлен PATH в {config_file}")
                added = True
                break
            except Exception as e:
                print(f"⚠️  Не удалось записать в {config_file}: {e}")
                continue
    
    if not added:
        # Создаём .bashrc если ничего не нашли
        bashrc = home / '.bashrc'
        try:
            with open(bashrc, 'a') as f:
                f.write('\n# Added by spotify-tui installer\n')
                f.write(path_line + '\n')
            print(f"✓ Создан {bashrc} с PATH")
            added = True
        except Exception as e:
            print(f"✗ Ошибка создания {bashrc}: {e}")
            return False
    
    return added

def setup_linux():
    """Настройка для Linux"""
    print("\n🐧 Обнаружена система: Linux")
    print("=" * 60)
    
    launcher = create_launcher_linux()
    print(f"✓ Создан launcher: {launcher}")
    
    # Определяем директорию для установки
    home = Path.home()
    local_bin = home / ".local" / "bin"
    
    # Создаём ~/.local/bin если не существует
    local_bin.mkdir(parents=True, exist_ok=True)
    print(f"✓ Директория {local_bin} готова")
    
    # Копируем launcher
    target = local_bin / "spotify-tui"
    try:
        shutil.copy2(launcher, target)
        os.chmod(target, 0o755)
        print(f"✓ Установлен в: {target}")
    except Exception as e:
        print(f"✗ Ошибка копирования: {e}")
        return False
    
    # Автоматически добавляем в PATH
    print("\n📝 Настройка PATH...")
    if add_to_path_linux(local_bin):
        print("✓ PATH настроен автоматически")
        print("\n🔄 Перезагрузите shell или выполните:")
        
        # Определяем команду для перезагрузки
        shell = os.environ.get('SHELL', '')
        if 'zsh' in shell:
            print("   source ~/.zshrc")
        elif 'fish' in shell:
            print("   source ~/.config/fish/config.fish")
        else:
            print("   source ~/.bashrc")
    else:
        print("⚠️  Не удалось автоматически настроить PATH")
        print(f"\n📋 Добавьте вручную в ~/.bashrc или ~/.zshrc:")
        print(f'   export PATH="$HOME/.local/bin:$PATH"')
    
    print(f"\n✅ Установка завершена!")
    print(f"🚀 Запуск: spotify-tui")
    return True

def add_to_path_windows(script_dir):
    """Автоматическое добавление в PATH для Windows"""
    try:
        # Получаем текущий PATH пользователя
        import winreg
        
        key = winreg.OpenKey(
            winreg.HKEY_CURRENT_USER,
            'Environment',
            0,
            winreg.KEY_READ | winreg.KEY_WRITE
        )
        
        try:
            current_path, _ = winreg.QueryValueEx(key, 'Path')
        except FileNotFoundError:
            current_path = ''
        
        # Проверяем, есть ли уже наша директория в PATH
        if str(script_dir) in current_path:
            print(f"✓ PATH уже содержит {script_dir}")
            winreg.CloseKey(key)
            return True
        
        # Добавляем нашу директорию
        if current_path and not current_path.endswith(';'):
            new_path = current_path + ';' + str(script_dir)
        else:
            new_path = current_path + str(script_dir)
        
        winreg.SetValueEx(key, 'Path', 0, winreg.REG_EXPAND_SZ, new_path)
        winreg.CloseKey(key)
        
        # Уведомляем систему об изменении
        import ctypes
        HWND_BROADCAST = 0xFFFF
        WM_SETTINGCHANGE = 0x1A
        ctypes.windll.user32.SendMessageW(HWND_BROADCAST, WM_SETTINGCHANGE, 0, 'Environment')
        
        print(f"✓ Директория {script_dir} добавлена в PATH автоматически")
        return True
        
    except Exception as e:
        print(f"⚠️  Не удалось автоматически добавить в PATH: {e}")
        return False

def setup_windows():
    """Настройка для Windows"""
    print("\n🪟 Обнаружена система: Windows")
    print("=" * 60)
    
    launcher = create_launcher_windows()
    print(f"✓ Создан launcher: {launcher}")
    
    script_dir = Path(__file__).parent.absolute()
    
    # Пытаемся автоматически добавить в PATH
    print("\n📝 Настройка PATH...")
    if add_to_path_windows(script_dir):
        print("✓ PATH настроен автоматически")
        print("\n⚠️  ВАЖНО: Перезапустите терминал для применения изменений!")
    else:
        print("⚠️  Не удалось автоматически настроить PATH")
        print(f"\n📋 Добавьте вручную:")
        print(f"   1. Win + R → sysdm.cpl")
        print(f"   2. Дополнительно → Переменные среды")
        print(f"   3. Path → Изменить → Создать")
        print(f"   4. Добавьте: {script_dir}")
        print(f"   5. ОК → Перезапустите терминал")
    
    print(f"\n✅ Установка завершена!")
    print(f"🚀 Запуск: spotify-tui")
    return True

def setup_macos():
    """Настройка для macOS"""
    print("\n🍎 Обнаружена система: macOS")
    print("=" * 60)
    
    # macOS использует ту же логику что и Linux
    return setup_linux()

def detect_and_setup():
    """Автоматическое определение ОС и запуск установки"""
    system = sys.platform
    
    if system == "win32":
        return setup_windows()
    elif system == "darwin":
        return setup_macos()
    elif system.startswith("linux"):
        return setup_linux()
    else:
        print(f"⚠️  Неизвестная система: {system}")
        print("Попытка установки как для Linux...")
        return setup_linux()

def main():
    print("=" * 60)
    print("🎵 SoundCloud TUI Player - Автоматический установщик")
    print("=" * 60)
    print()
    
    # Установка зависимостей
    if not install_dependencies():
        print("\n❌ Установка прервана из-за ошибок")
        sys.exit(1)
    
    # Автоматическое определение ОС и настройка
    if not detect_and_setup():
        print("\n❌ Установка не завершена")
        sys.exit(1)
    
    print("\n" + "=" * 60)
    print("🎉 Всё готово! Перезапустите терминал и используйте: spotify-tui")
    print("=" * 60)

if __name__ == "__main__":
    main()
