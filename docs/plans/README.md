# Milestone plans

Một file cho một milestone. Viết plan ở đây trước khi code. Plan viết bằng tiếng Việt;
tài liệu hướng người dùng (README, GUIDE) viết tiếng Anh.

| Milestone | Trạng thái | Tóm tắt |
|---|---|---|
| [M0: Scaffold](M0-scaffold.md) | Xong | Workspace 3 crate, cửa sổ GPUI hiện chữ trên macOS, CI build Windows, repo public |
| [M1: Core + Audio](M1-core-audio.md) | Xong | Quét thư viện (jwalk + lofty + rusqlite), engine cpal + symphonia có gapless + ReplayGain, CLI smoke |
| [M2: Library UI](M2-library-ui.md) | Đợt A xong, đợt B đang làm | Sidebar, danh sách ảo, album grid có ảnh bìa, thanh Now Playing, seek/volume, tìm kiếm |
| M3: Windows integration | Chưa | Phím media + SMTC qua souvlaki, theo dõi thư mục (notify), settings, bản release + bộ cài |
| M4: Extras | Chưa | Playlist, lời LRC, mini player, Last.fm, Opus |

## Quan hệ với Cadence

`Code/Personal/windows` là **Cadence** (Avalonia 12 + BASS, repo `duchuynguyen93/cadence`), cùng ý tưởng
music player local cho Windows. Aurum là project mới, viết lại bằng Rust + GPUI, **không** kế thừa
ngôn ngữ thiết kế Ink/Amber của Nocturne/Cadence mà đi hướng sang trọng riêng (xem `docs/DESIGN.md`).
Cadence để nguyên, không đụng.

## Ranh giới đã chốt

- Target chính Windows, dev và chạy thử hằng ngày trên macOS. Pixel giống nhau vì GPUI tự vẽ.
- Windows chỉ được kiểm chứng qua CI `windows-latest` cho đến khi user chạy trên máy bàn ở nhà.
- Không Opus ở v1 (symphonia 0.6 không decode Opus). Không WASAPI exclusive. Không tray, không hotkey toàn cục (GPUI không có).
