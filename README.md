# yandex-music-downloader

Консольная программа на Rust для загрузки доступной вам музыки из Яндекс.Музыки. Проект создан на основе [оригинального yandex-music-downloader](https://github.com/llistochek/yandex-music-downloader) и независим от компании Яндекс.

## Возможности

- Загрузка трека, альбома, альбомов исполнителя или плейлиста по ID или ссылке.
- Качество `0` (низкое), `1` (обычное) и `2` (лучшее, включая FLAC при наличии).
- Теги и обложки для MP3, FLAC и M4A; текст песни внутри файла или отдельный LRC.
- Шаблон пути, пропуск существующих файлов, фильтрация альбомов исполнителя и повторы сетевых запросов.

## Установка

Можно скачать готовый архив для Linux x86_64/aarch64, Windows x86_64 или macOS arm64/x86_64 со страницы [GitHub Releases](https://github.com/Kritile/yandex-music-downloader-rust/releases). Linux-архивы собраны с glibc. Распакуйте архив и запустите `yandex-music-downloader` (`yandex-music-downloader.exe` на Windows).

Для сборки из исходников нужен Rust с Cargo. Python и FFmpeg для запуска не требуются.

```sh
git clone https://github.com/Kritile/yandex-music-downloader-rust.git
cd yandex-music-downloader-rust
cargo install --path .
yandex-music-downloader --help
```

Для запуска без установки: `cargo run --release -- --help`. Готовый файл после `cargo build --release` находится в `target/release/yandex-music-downloader`.

## Авторизация

Программа принимает OAuth-токен из переменной окружения `YANDEX_MUSIC_TOKEN` или через `--token`. Если заданы оба, приоритет у `--token`. Инструкция по получению токена: [ym.marshal.dev/token](https://ym.marshal.dev/token/#implicit-oauth).

Для ввода без отображения токена и записи его значения в историю Bash:

```bash
read -rsp 'OAuth token: ' YANDEX_MUSIC_TOKEN
printf '\n'
export YANDEX_MUSIC_TOKEN
yandex-music-downloader --track-id 6705392
unset YANDEX_MUSIC_TOKEN
```

Не вводите токен прямо в строке `YANDEX_MUSIC_TOKEN=... yandex-music-downloader ...`: такая строка может сохраниться в истории оболочки. Переменная окружения не показывается в обычном `ps aux`, но доступ к окружению процесса у пользователя с соответствующими правами остаётся возможным.

## Примеры

```sh
yandex-music-downloader --quality 2 --url "https://music.yandex.ru/artist/208167"
yandex-music-downloader --quality 1 --lyrics-format lrc --url "https://music.yandex.ru/album/294912"
yandex-music-downloader --url "https://music.yandex.ru/album/11644078/track/6705392"
yandex-music-downloader --playlist-id "<владелец>/<тип>" --skip-existing
```

## Параметры

Источник обязателен: ровно один из `--artist-id`, `--album-id`, `--track-id`, `--playlist-id <владелец>/<тип>` или `-u, --url`. Поддерживаются ссылки на страницы исполнителя, альбома, трека и плейлиста. Токен обязателен; его можно передать через `YANDEX_MUSIC_TOKEN` или `--token`.

| Параметр | Назначение | По умолчанию |
| --- | --- | --- |
| `--token <токен>` | OAuth-токен; имеет приоритет над `YANDEX_MUSIC_TOKEN` | из окружения |
| `--quality 0..2` | Качество аудио | `0` |
| `--skip-existing` | Пропускать файл, если уже есть MP3, FLAC или M4A с тем же базовым путём | выключено |
| `--lyrics-format none\|text\|lrc` | Без текста, текст в теге или отдельный LRC; для LRC без синхронного текста используется обычный текст | `none` |
| `--embed-cover` | Встроить обложку; иначе создать `cover.jpg` или `cover.png` | выключено |
| `--cover-resolution 400\|original` | Сторона квадратной обложки от 100 пикселей либо оригинал | `400` |
| `--delay <секунды>` | Пауза после трека | `0` |
| `--stick-to-artist` | Для исполнителя брать только альбомы, где он указан первым | выключено |
| `--only-music` | Для исполнителя пропускать подкасты и другие немузыкальные альбомы | выключено |
| `--compatibility-level 0\|1` | Способ записи нескольких артистов в M4A | `1` |
| `--timeout <секунды>` | Время ожидания ответа | `20` |
| `--tries <число>` | Число повторов при сетевой ошибке; `0` — без ограничения | `20` |
| `--retry-delay <секунды>` | Пауза между повторами | `5` |
| `--dir <путь>` | Корневой каталог загрузки | `.` |
| `--path-pattern <шаблон>` | Относительный путь к треку без расширения | `#album-artist/#album/#number - #title` |
| `--unsafe-path` | Сохранять больше символов в названиях файлов | выключено |

Шаблон пути поддерживает `#number`, `#number-padded`, `#disc-number`, `#disc-number-padded`, `#track-artist`, `#album-artist`, `#title`, `#album`, `#year`, `#artist-id`, `#album-id`, `#track-id`. Для многодискового альбома `#number` включает номер диска, если шаблон не содержит `#disc-number`.

Скрытый устаревший флаг `--add-lyrics` продолжает работать как `--lyrics-format text`. Полный актуальный список опций выводит `--help`.

### Уровни совместимости тегов

Уровень `0` записывает несколько исполнителей в M4A отдельными значениями. Уровень `1` объединяет их через `; ` для совместимости с большим числом плееров. Для остальных форматов уровень не меняет набор тегов.

## Переход с Python-версии

Начиная с версии 4.0.0 программа собирается через Cargo. Флаги командной строки сохранены. История изменений и инструкция по переходу: [MIGRATIONS.md](MIGRATIONS.md). Устройство Rust-версии описано в [ARCHITECTURE.md](ARCHITECTURE.md).

## Использование генеративного ИИ

Перенос программы на Rust и обновление документации выполнены с использованием генеративного ИИ. Изменения проверялись сборкой, статическим анализом, автоматическими тестами и пробной загрузкой с временным токеном.

## Благодарности

- Разработчикам [yandex-music-api](https://github.com/MarshalX/yandex-music-api).
- @ArtemBay за [пример получения ссылки на lossless](https://github.com/MarshalX/yandex-music-api/issues/656#issuecomment-2306542725).
- @keltecc за [метод расшифровки](https://github.com/llistochek/yandex-music-downloader/issues/112#issuecomment-2812535100).
- @leowerd за [исправление имён исполнителей](https://github.com/llistochek/yandex-music-downloader/issues/93#issuecomment-2960210879).

## Дисклеймер

Проект является независимой разработкой и никак не связан с компанией Яндекс.
