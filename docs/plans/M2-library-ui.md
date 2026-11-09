# M2 — Library UI

Mục tiêu: app dùng được hằng ngày để nghe nhạc trong thư viện local: thêm thư mục, quét, duyệt album/nghệ sĩ/bài,
phát với Now Playing bar đầy đủ, tìm kiếm. Giao diện theo `docs/DESIGN.md`. Chia hai đợt, review giữa hai đợt.

## Thành phần dùng từ gpui-component 0.9 (đã có trong source registry)
`sidebar`, `virtual_list`/`list`, `scroll`, `input`, `slider.rs`, `button`, `progress`, `menu`, `dialog`, `theme`,
`title_bar.rs`, `empty.rs`, `skeleton.rs`, `tooltip.rs`. GPUI core: `uniform_list`, `img` + `ImageSource::Image(Arc<Image>)`,
`cx.prompt_for_paths` (app.rs:2039) cho hộp thoại chọn thư mục, `background_executor().timer`, `text_system().add_fonts`.

## Dữ liệu và trạng thái (`crates/aurum-app/src/state/`)

- `paths.rs`: thư mục cấu hình và dữ liệu qua crate `dirs 6`: config `aurum/settings.json`, data `aurum/library.db`.
  Windows: `%APPDATA%\aurum`, macOS: `~/Library/Application Support/aurum`.
- `settings.rs`: `Settings { roots: Vec<PathBuf>, volume: f32, shuffle: bool, repeat: Repeat }`, serde_json, ghi atomically
  (ghi file tạm rồi rename).
- `app_state.rs`: một `Entity<AppState>` toàn cục (đặt qua `cx.set_global` hoặc truyền xuống view):
  - `library: Library` (connection reader), `albums: Vec<Album>` cache, `tracks: Vec<Track>` cache, làm mới sau mỗi scan.
  - `queue: aurum_core::Queue`, `engine: Box<dyn AudioEngine>` (CpalEngine; lỗi mở device thì NullEngine kèm banner lỗi).
  - `playback: Playback { playing: bool, position: Duration, duration: Duration, volume: f32, current: Option<Track> }`.
  - `scan: Option<ScanProgress>`.
  - `view: View` enum: `Albums`, `AlbumDetail{artist, album}`, `Artists`, `ArtistDetail{name}`, `Tracks`, `Settings`.
  - `search: String`.
- `bridge.rs`: bơm event từ engine vào GPUI. Một task foreground `cx.spawn(async move |this, cx| loop { timer(50ms).await; drain try_recv; this.update(cx, ...) })`.
  Xử lý: `Position` → playback.position; `TrackEnded` → `queue.next()`; `TrackStarted(path)` → playback.current = track khớp path,
  rồi `engine.queue_next(queue.peek_next())`; `Loaded` → duration; `Error` → banner.
  Scan: `ScanEvent` receiver bơm cùng cách, `Finished` → reload albums/tracks từ DB.
- Thao tác phát: `play_album(artist, album, start_index)`, `play_tracks(tracks, start)`, `toggle_play`, `next`, `prev`
  (prev khi position > 3s thì seek 0), `seek(ratio)`, `set_volume`, `toggle_shuffle`, `cycle_repeat`.
  Sau mỗi lần đổi bài: `engine.load(path, rg)` + `engine.play()` + `engine.queue_next(peek_next)`.
  ReplayGain: dùng track gain, peak track; thiếu thì None.

Cần thêm ở `aurum-core::Queue` (được phép sửa core, giữ test cũ xanh, thêm test mới): `replace(tracks: Vec<Track>, start: usize)`,
`peek_next(&self) -> Option<&Track>` (tôn trọng repeat/shuffle, không đổi trạng thái), `jump(index)`, `tracks()` iterator theo thứ tự phát,
`position() -> Option<usize>`.

## Đợt A — chạy được (M2a)

1. State + bridge + settings như trên. Font bundle: tải Fraunces (variable, OFL), Inter (variable, OFL), JetBrains Mono (OFL)
   vào `crates/aurum-app/assets/fonts/` kèm file OFL.txt của từng font, `include_bytes!` và `add_fonts` lúc khởi động.
   Fallback khi tên font không có: GPUI tự rơi về font hệ thống.
2. Shell: sidebar 220px (wordmark AURUM, nhóm "Library": Albums, Artists, Tracks; dưới cùng: Settings), vùng chính, Now Playing bar 88px.
   Mục nav đang chọn: chấm Gold 6px bên phải, chữ Ivory50; hover: nền Obsidian800.
3. View **Tracks**: `uniform_list` cao 44px mỗi dòng: [số thứ tự hoặc icon đang phát] [title Ivory100 15px] [artist Ivory300 13px] [album] [duration mono].
   Hover: nền Obsidian800. Double-click: `play_tracks(all_tracks, index)`. Dòng đang phát: title màu Gold500.
4. View **Settings**: danh sách roots, nút "Add folder" (`prompt_for_paths` chọn thư mục, cho phép nhiều), nút xoá từng root, nút "Rescan",
   thanh tiến độ khi đang quét (`scanned/total_hint`, số added). Quét tự động khi khởi động nếu có roots.
5. **Empty state** khi chưa có root: giữa vùng chính, tiêu đề Fraunces 34px "Your library is empty", mô tả, nút Gold "Add a music folder".
6. **Now Playing bar**: trái: ảnh bìa 56px bo 10 (placeholder Obsidian800 khi chưa có), title (Fraunces 17px, Ivory50), "Artist · Album" (13px Ivory300).
   Giữa: shuffle, prev, play/pause (tròn 44px Gold500, glyph Obsidian950), next, repeat; dưới là progress: thời gian mono 11px hai bên,
   track 2px Obsidian700, fill Gold500, handle 10px khi hover, click/drag để seek. Phải: icon loa + slider volume 96px.
   Icon: dùng `gpui_component::Icon` với SVG inline (lucide) đặt trong `assets/icons/`.
7. Phím tắt: Space play/pause (khi focus không ở input), Cmd/Ctrl+Right/Left next/prev, Cmd/Ctrl+, mở Settings.
8. Ảnh bìa: `cover_cache.rs`, `HashMap<(artist, album), Arc<Image>>`, nạp ở background executor từ `cover::load(first_track_path)`,
   decode sang `Image { format, bytes }` (GPUI tự decode PNG/JPEG qua `ImageFormat`), giới hạn 200 mục, LRU đơn giản.

Định nghĩa xong M2a: trên Mac thêm `~/Music`, quét xong hiện danh sách bài, double-click phát ra loa, bar hiện đúng title/artist, progress chạy,
seek và volume hoạt động, hết bài tự sang bài kế (gapless). Chụp 3 ảnh: empty state, Tracks đang phát, Settings đang quét. CI xanh cả hai job.

## Đợt B — duyệt đẹp (M2b)

1. View **Albums**: lưới card, cột tự tính theo chiều rộng (`min 160px`, gap 24), mỗi card: bìa vuông bo 14 có bóng, tên album Fraunces 15px,
   nghệ sĩ 13px Ivory300, năm. Dùng `uniform_list` theo hàng (mỗi hàng N card) để ảo hoá. Click → AlbumDetail.
2. View **AlbumDetail**: header: bìa 240px bo 20 bóng mềm + nền blur từ bìa (tối 70%), tên album Fraunces 34px, nghệ sĩ, năm · số bài · tổng thời lượng,
   nút Gold "Play" và nút viền "Shuffle". Dưới là danh sách bài (số, title, duration) gom theo disc nếu > 1 disc. Nút back.
3. View **Artists**: danh sách tên + số album; click → lưới album lọc theo nghệ sĩ.
4. **Tìm kiếm**: ô input góc phải trên (gpui-component `input`), Cmd/Ctrl+F focus, lọc view hiện tại theo `Library::search`, Esc xoá.
5. Nền Now Playing bar blur bìa bài đang phát trộn Obsidian900 (nhẹ, 85% tối).
6. Title bar tuỳ biến bằng `title_bar.rs` của gpui-component để Windows không hiện thanh trắng mặc định (macOS giữ traffic lights).
7. Polish: shimmer/skeleton khi đang nạp bìa, tooltip cho nút, trạng thái lỗi engine hiện banner Danger ở trên bar.

Định nghĩa xong M2b: chụp 4 ảnh Albums, AlbumDetail, Artists, Search đang lọc; ảnh phải được tự nhìn lại và so với DESIGN.md; CI xanh.

## Rủi ro
- `uniform_list` cần chiều cao dòng cố định, card album phải cố định chiều cao theo chiều rộng cột tính trước.
- Blur ảnh: GPUI không có filter blur trực tiếp; làm bằng crate `image` resize xuống 32px rồi phóng to với `object_fit` + lớp phủ tối (blur rẻ).
  Nếu không đạt thì dùng gradient từ màu trung bình của bìa.
- `prompt_for_paths` trả `Receiver<Result<Option<Vec<PathBuf>>>>`, phải await trong spawn.
- Trên Windows, callback cpal và GPUI chạy khác thread, mọi thứ đi qua channel như thiết kế M1 nên không có vấn đề, nhưng chưa ai chạy.
