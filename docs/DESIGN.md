# Aurum — Design direction

Hướng: **hi-fi sang trọng**, kiểu máy nghe nhạc cao cấp hơn là app streaming. Yên tĩnh, nhiều khoảng thở,
ảnh bìa là nhân vật chính, một màu nhấn vàng champagne dùng tiết chế. Không mượn Fluent của Windows,
không mượn Ink/Amber của Nocturne.

## Màu

| Token | Hex | Dùng cho |
|---|---|---|
| `Obsidian950` | `#0C0B0E` | nền cửa sổ |
| `Obsidian900` | `#121114` | vùng lõm, sidebar |
| `Obsidian850` | `#1A181C` | card, panel |
| `Obsidian800` | `#222025` | bề mặt nổi, hover row |
| `Obsidian700` | `#2E2B31` | viền |
| `Obsidian600` | `#45414A` | viền đậm, disabled fill |
| `Ivory500` | `#8E887E` | text mờ |
| `Ivory300` | `#B9B2A6` | text phụ |
| `Ivory100` | `#EFE9DE` | text chính |
| `Ivory50`  | `#F7F3EA` | tiêu đề |
| `Gold500`  | `#D4AF6A` | nút play, progress fill, focus ring, active nav |
| `Gold400`  | `#E6C680` | hover của gold |
| `Gold700`  | `#A8853F` | gold pressed, viền gold mờ |
| `Success`  | `#7FB58A` | xác nhận |
| `Danger`   | `#C96B6B` | lỗi |

Nền hơi ấm, không phải xám trung tính: `#0C0B0E` lệch tím ấm một chút. Text là ngà, không trắng tinh.
Gold chỉ xuất hiện ở: nút play, phần đã phát của progress, mục nav đang chọn, focus ring. Mọi thứ khác
dùng ramp Obsidian/Ivory.

## Chữ

| Vai trò | Font | Fallback |
|---|---|---|
| Display (tên album, tiêu đề view, tên bài ở Now Playing) | Fraunces (OFL, serif có optical size) | Georgia / serif hệ thống |
| UI (danh sách, nhãn, nút) | Inter (OFL) | Segoe UI trên Windows, SF trên macOS |
| Mono (timecode, bitrate, sample rate) | JetBrains Mono (OFL) | Consolas / Menlo |

Font bundle vào binary qua `include_bytes!` và nạp bằng `cx.text_system().add_fonts(...)`.
Thiếu font thì app vẫn đọc được, chỉ mất chất display.

Cỡ chữ: 11 (meta), 13 (UI), 15 (row title), 22 (view title), 34 (Now Playing track title, Fraunces).
Letter-spacing: display hơi âm (-0.01em), nhãn mục nav viết hoa nhỏ tracking +0.08em.

## Hình khối

- Radius: 6 (chip), 10 (row, input), 14 (card, ảnh bìa nhỏ), 20 (ảnh bìa lớn, panel nổi).
- Spacing cơ bản 8. Padding panel 24, 32 ở Now Playing.
- Divider: hairline 1px `Ivory50` alpha 6%.
- Shadow: ảnh bìa lớn có bóng mềm 0 24 48 alpha 45%, chỉ ảnh bìa mới có bóng.
- Nút play: hình tròn 56px fill `Gold500`, glyph `Obsidian950`. Là hình đặc duy nhất trên bề mặt.
- Progress: track 2px `Obsidian700`, fill `Gold500`, handle 10px chỉ hiện khi hover.
- Nền Now Playing: ảnh bìa blur mạnh (radius 80), tối 70%, trộn vào `Obsidian950`. Đây là chỗ duy nhất có gradient.

## Bố cục chính (M2)

```
┌─────────────┬─────────────────────────────────────────────┐
│ AURUM       │  Albums                          [search ]  │
│             │                                              │
│ Library     │  ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐          │
│  Albums  ●  │  │    │ │    │ │    │ │    │ │    │  grid   │
│  Artists    │  └────┘ └────┘ └────┘ └────┘ └────┘          │
│  Tracks     │   Title  Title  Title  Title  Title          │
│  Folders    │   Artist Artist Artist Artist Artist         │
│             │                                              │
│ Playlists   │                                              │
│  + New      │                                              │
├─────────────┴─────────────────────────────────────────────┤
│ [art] Track title (Fraunces)   ⏮  (▶)  ⏭      0:42 ──●──── 4:13   🔊 ──── │
│       Artist · Album                                                    │
└──────────────────────────────────────────────────────────────────────────┘
```

Sidebar 220px, nền `Obsidian900`. Bar dưới 88px, nền `Obsidian900`, divider trên.
Mục nav đang chọn: chấm `Gold500` 6px bên phải, text `Ivory50`; không có fill nền.
