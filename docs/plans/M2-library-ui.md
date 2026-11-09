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

## Ghi chú đợt A

API thật đã dùng (gpui-pre 0.3.11, gpui-component 0.9, đọc từ registry):
- Assets: `gpui_kit::application().with_assets(assets::Assets)`; `Assets` (`src/assets.rs`) là `AssetSource` tự viết, nhúng 17 SVG lucide trong `assets/icons/` bằng `include_bytes!`, path `icons/<tên>.svg`, không thấy thì rơi về `gpui_kit::assets::Assets`. Lucide đổi tên `trash-2` thành `trash`. `play.svg`/`pause.svg` đã đổi `fill="none"` thành `fill="currentColor"` để glyph đặc. Icon vẽ bằng `svg().path(..).text_color(..)` của GPUI, không dùng `gpui_component::Icon`.
- Font: `cx.text_system().add_fonts(vec![Cow::Borrowed(bytes)])` trước khi mở cửa sổ. Fraunces, Inter, JetBrains Mono là variable font, macOS nhận được (chữ Fraunces serif khác hẳn Inter, mono đúng). Fraunces ra khá đậm ở cỡ nhỏ (trục opsz/wght mặc định), chưa chỉnh.
- Slider: không dùng `gpui_component::slider`; tự vẽ (`ui/slider.rs`) bằng `canvas` đo bounds, `on_mouse_down` + `on_drag` + `on_drag_move::<D>` (mỗi slider một kiểu D riêng, vì drag-move định tuyến theo TypeId) + `on_mouse_up`/`on_mouse_up_out`. Handle hiện bằng `group_hover`.
- Danh sách: `uniform_list` 44px, `cx.processor`; dòng phải `.w_full()` nếu không cột `relative()` co theo nội dung. Double-click: `ClickEvent::click_count() >= 2`.
- Hộp thoại thư mục: `cx.prompt_for_paths(PathPromptOptions{directories:true, multiple:true,..})`, await trong `cx.spawn`. Chưa bấm thử hộp thoại hệ thống; settings.json được tạo tay với root `~/Music`.
- Phím tắt: `actions!` của gpui_kit, `KeyBinding::new` với context "Aurum" (chọn cmd/ctrl bằng `cfg!`, vì `secondary-` cần `parse_with_os`). Click nền gọi `window.focus` để phím tắt không mất.

Lệch plan:
- `AppState` vừa là model vừa là root view (impl `Render` ở `ui/shell.rs`), không tách `Entity<AppState>` riêng. Không giữ `Library` trong state: mỗi lần reload mở connection mới ở background executor.
- Không có `state/bridge.rs`: bridge nằm trong `AppState::tick` (timer 50ms). Không có `search` (đợt B). `View` chỉ có Albums/Artists/Tracks/Settings; Albums, Artists là placeholder. Mặc định mở Tracks.
- Wordmark dùng Ivory50 (DESIGN.md chỉ cho Gold ở vài chỗ), không Gold như M0.
- Thêm biến môi trường `AURUM_HOME` (ghi đè thư mục config + data, dùng cho test và chụp ảnh).
- Thêm dep `rand 0.9` cho shuffle, `crossbeam-channel` cho `ScanEvent`.
- Hàng đợi scan: yêu cầu quét lúc đang quét sẽ lặp lại một lần sau khi quét xong. Xoá root chạy quét để scanner dọn dòng của root đó.

Hạn chế:
- `AudioEngine` không có lệnh xoá hàng chờ: đổi repeat One -> Off ở bài cuối vẫn có thể lặp thêm một lần bài đã preload.
- Khi cửa sổ bị che hoàn toàn, GPUI ngừng vẽ nên progress đứng hình (chạy lại khi hiện).
- Đã thử bằng chuột thật: double-click phát, click/kéo seek trên progress. Chưa thử kéo volume, bài tự chuyển gapless, và chưa chụp được ảnh Settings đang quét (quét 326 file xong dưới 1 giây).
- Tên cột artist/album trống với các bài không có tag (thư mục Unknown Artist).

## Ghi chú đợt B

API thật đã dùng (gpui-pre 0.3.11, gpui-component 0.9):
- **Theme**: `theme::apply(cx)` gọi `Theme::change(ThemeMode::Dark)` rồi `Theme::update` để đặt background, foreground, muted_foreground, border, input, ring (Gold500), caret, selection, popover, skeleton, font_family, radius từ token. Widget của gpui-component (Input, Tooltip, Skeleton) đọc màu từ đây.
- **Ảnh bìa**: `cover_cache` giờ giữ `CoverArt { thumb, blur }`, cả hai là `Arc<RenderImage>` dựng ở background executor bằng crate `image` (feature jpeg + png): thumb tối đa 400px, blur = resize 64x64 rồi `imageops::blur(sigma 4)`. Pixel đổi sang BGRA trước khi vào `RenderImage::new`. Không đưa ảnh full-size cho GPUI (101 album x ảnh vài megapixel sẽ ăn RAM/texture). Bìa WebP chưa hỗ trợ (hiện placeholder đĩa), bìa nằm ngoài tag (folder.jpg) chưa đọc: M1 `cover::load` chỉ đọc ảnh nhúng. `Lookup::{Ready, Missing, Loading}`; Loading vẽ `gpui_component::skeleton::Skeleton` (nhấp nháy opacity), Missing vẽ icon disc trên Obsidian800.
- **Blur nền**: ảnh blur 64px được kéo giãn bằng `img(..).absolute().size_full().object_fit(Cover)`; GPUI lấy mẫu song tuyến nên phóng to từ ảnh đã blur thì mịn, không lộ ô vuông (đã nhìn ảnh chụp). Bar: lớp phủ Obsidian900 85%. AlbumDetail header: lớp phủ Obsidian950 70% + dải gradient 96px dưới đáy hoà vào nền trang (lệch nhẹ DESIGN "gradient chỉ ở Now Playing", ghi nhận). Không cần fallback màu trung bình.
- **Lưới Albums**: số cột tính từ `window.viewport_size().width - 220 - 64` (không cần canvas đo, không giật một khung). `uniform_list` theo hàng, chiều cao hàng = card + 12 + 54 + 24 cố định. Cover được nạp trong `cx.processor` (chỉ hàng đang hiện). Dùng chung cho ArtistDetail.
- **Điều hướng**: `View` vẫn là Albums/Artists/Tracks/Settings (Copy); trang chi tiết là `AppState.detail: Vec<Detail>` (Album/Artist) chồng lên view. Mở album từ trang nghệ sĩ rồi Back quay về trang nghệ sĩ; bấm mục sidebar thì xoá hết chồng. Mặc định mở Albums.
- **Artists**: tính ở app từ `all_tracks()` (nhóm theo album_artist), không thêm query vào `aurum-core` (không đụng `db.rs`/`mod.rs`, nên không có test core mới). Test `artists_from` nằm ở `state/app_state.rs`. Track không có album_artist không vào danh sách nghệ sĩ.
- **Tìm kiếm**: `gpui_component::input::{Input, InputState, InputEvent}`; `InputState::new(window, cx)` nên `AppState::new(window, cx)`. `Library::search` chạy ở background, có bộ đếm thế hệ để bỏ kết quả cũ. Tracks: lọc theo hit; Albums: tên/nghệ sĩ khớp (Rust, không phân biệt hoa thường) hoặc có bài nằm trong hit; Artists: lọc theo tên. Cmd/Ctrl+F → `FocusSearch`; Esc: Input tự gọi `cx.propagate()` nên binding `escape` trên context "Aurum" (`ClearSearch`) chạy cả khi đang gõ; `set_value("")` không phát Change nên tự reset trạng thái. Space/Next/Prev: binding nằm trên "Aurum" (chứa cả Input; `!Input` không giúp vì GPUI so khớp ở từng độ sâu) nên handler kiểm `self.focus.is_focused(window)`, không thì `cx.propagate()`; đã gõ được "andrea bo" có dấu cách mà nhạc không bị toggle. Ô tìm kiếm chỉ hiện ở Albums/Artists/Tracks/ArtistDetail, không ở AlbumDetail.
- **Title bar**: `WindowOptions { titlebar: TitlebarOptions { appears_transparent: true, traffic_light_position: (16,12) }, ..TitleBar::window_options() }` (có `app_owns_titlebar_drag: true`, cả content view là vùng app tự kéo). Sidebar chạy tận mép trên; dải kéo 34px của sidebar là `drag_strip` tự viết (mouse-down bật cờ, mouse-move đầu tiên gọi `window.start_window_move()`, double-click `titlebar_double_click`/`zoom_window`, `window_control_area(Drag)`); cột chính dùng `gpui_component::TitleBar` (cho nút min/max/close của Windows) phủ tuyệt đối trên cùng, nền Obsidian950 (trong suốt ở trang album để thấy blur). Đã thử kéo bằng chuột thật trên cả hai dải: cửa sổ dịch đúng. Windows chưa chạy thử, chỉ dựa vào mã của `TitleBar`.
- **Polish**: tooltip bằng `div().tooltip(|w, cx| Tooltip::new(..).build(w, cx))` của GPUI (managed tooltip của gpui-component là `pub(crate)`); banner Danger trên bar khi `engine_error` (nút x xoá), đã chụp bằng cách ép lỗi tạm thời qua biến môi trường (đã gỡ).

Lệch plan:
- Không có `View::AlbumDetail/ArtistDetail` mà dùng chồng `detail`; không có `artists()`/`albums_of_artist()` ở core; tìm kiếm Artists lọc theo tên thôi.
- Thêm dep `image 0.25` (jpeg, png; đã có sẵn trong Cargo.lock qua gpui nên không build thêm).
- Trong module test không dùng `use super::*` nếu file có `use gpui_kit::*`: glob kéo theo macro `test` của gpui, đè `#[test]` và gây "recursion limit reached".

Hạn chế:
- Thư viện thử nghiệm có nhiều bản trùng (mỗi bài hai file) nên album hiển thị hai dòng cùng tên.
- Texture của ảnh bìa không được giải phóng khỏi atlas GPUI khi bị đẩy khỏi cache LRU (RenderImage không tự `drop_image`); 101 album ~ 65MB, chấp nhận được.
- Khi cửa sổ bị che hoàn toàn GPUI ngừng vẽ nên ảnh chụp lúc đó là ảnh cũ; khi chụp phải đưa cửa sổ lên trên cùng.
- Albums không có bìa nhúng hiện icon đĩa (khoảng 1/4 album thử nghiệm).
- Fraunces vẫn khá đậm ở 15px (trục opsz/wght mặc định).
