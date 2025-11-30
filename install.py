import os
import sys
import subprocess

# Все зависимости из main.py
libs = [
    "textual",
    "pygame-ce",
    "yt-dlp",
    "requests"
]


def install_package(package):
    """Установка пакета с выводом статуса"""
    try:
        print(f"Устанавливаю {package}...")
        subprocess.check_call(
            [sys.executable, "-m", "pip", "install", package],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE
        )
        print(f"✓ {package} успешно установлен")
        return True
    except subprocess.CalledProcessError as e:
        print(f"✗ Ошибка при установке {package}: {e}")
        return False


if __name__ == "__main__":
    print("=" * 50)
    print("Установка зависимостей для SoundCloud TUI Player")
    print("=" * 50)
    print()

    failed = []
    for lib in libs:
        if not install_package(lib):
            failed.append(lib)
        print()

    print("=" * 50)
    if not failed:
        print("✓ Все зависимости успешно установлены!")
        print("Теперь можно запустить: python main.py")
    else:
        print("✗ Не удалось установить следующие пакеты:")
        for lib in failed:
            print(f"  - {lib}")
        print("\nПопробуйте установить их вручную:")
        print(f"  {sys.executable} -m pip install {' '.join(failed)}")
    print("=" * 50)
