# M1 — Core + Audio

Mục tiêu: quét một thư mục nhạc vào SQLite, phát được một chuỗi file từ dòng lệnh với gapless thật và
ReplayGain, có test tự động cho cả quét lẫn gapless mà không cần tai người. Chưa đụng UI.

## Phạm vi

Có: FLAC, MP3, AAC/ALAC trong MP4 (.m4a), Vorbis (.ogg/.oga), WAV, AIFF. Không: Opus, WASAPI exclusive, CUE sheet.

## Core (`aurum-core`)

Dependency: `jwalk`, `lofty 0.25`, `rusqlite 0.41` (features `bundled`), `thiserror`, `crossbeam-channel`.

### Quét: `library/scan.rs`
- `Scanner::scan(roots: &[PathBuf], tx: Sender<ScanEvent>)` chạy trên thread nền. jwalk duyệt song song,
  lọc theo đuôi `flac mp3 m4a aac ogg oga wav aif aiff`.
- Mỗi file: `lofty::read_from_path`, lấy title, artist, album_artist, album, track_no, disc_no, year, genre,
  duration_ms, sample_rate, channels, codec (từ `FileType`), ReplayGain 4 giá trị từ `ItemKey::ReplayGainTrackGain`
  `ReplayGainTrackPeak` `ReplayGainAlbumGain` `ReplayGainAlbumPeak` (parse chuỗi "-6.20 dB" → f32; thiếu thì None).
  Thiếu title thì dùng tên file không đuôi. Thiếu album_artist thì dùng artist.
- Incremental: so `mtime + size` với DB, file không đổi thì bỏ qua. File trong DB mà không còn trên đĩa thì xoá.
- Ảnh bìa **không** lưu DB. Hàm riêng `cover::load(path) -> Option<Cover{mime, bytes}>` đọc ảnh đầu tiên qua lofty,
  M2 sẽ cache theo album trong bộ nhớ.
- `ScanEvent`: `Started{total_hint}`, `Progress{scanned, added, updated}`, `Finished{summary}`, `Error{path, msg}`.

### DB: `library/db.rs`
- `Library::open(path)`; schema qua `PRAGMA user_version`, migration v1:
  `tracks(id INTEGER PK, path TEXT UNIQUE, title, artist, album_artist, album, track_no, disc_no, year, genre,
  duration_ms, sample_rate, channels, codec, rg_track_gain, rg_track_peak, rg_album_gain, rg_album_peak, mtime, size)`.
  Index trên `(album_artist, album)` và `artist`.
- Query: `all_tracks()`, `albums()` (GROUP BY album_artist, album → tên, nghệ sĩ, năm, số bài, path bài đầu để lấy bìa),
  `tracks_of_album(album_artist, album)` sắp theo disc_no, track_no, `search(q)` LIKE trên title/artist/album.
- `upsert(track)`, `remove_missing(seen_paths)`, `stat(path) -> Option<(mtime,size)>`.
- WAL mode, một connection cho writer ở thread quét, reader mở connection riêng (SQLite cho phép).

### Test core
- Fixture sinh bằng ffmpeg trong `scripts/make-fixtures.sh` (sine 440Hz và 880Hz, 2 giây, FLAC + MP3 + M4A + WAV),
  commit vào `tests/fixtures/` (mỗi file dưới 100KB). Script ghi tag bằng ffmpeg `-metadata`.
- Test: quét fixture ra đúng số bài và tag; quét lần hai không có added/updated; xoá một file rồi quét thì DB bớt
  một; parse ReplayGain từ chuỗi; album grouping với album_artist thiếu.

## Audio (`aurum-audio`)

Dependency: `symphonia 0.6` features `mp3 aac isomp4 alac flac vorbis wav aiff`, `cpal 0.18`, `rubato 5`,
`rtrb` (ring buffer SPSC lock-free), `crossbeam-channel`, `thiserror`, `log`.

### Kiến trúc

```
UI/CLI ──Command──▶ DecoderThread ──f32 interleaved @out_rate──▶ rtrb ring ──▶ Sink callback ──▶ loa
            ◀──Event──┘   (symphonia → gain → rubato)                       (CpalSink | WavSink)
```

- `Command`: `Load(path)`, `Play`, `Pause`, `Seek(Duration)`, `SetVolume(f32)`, `QueueNext(Option<path>)`, `Stop`.
- `Event`: `Loaded{path, duration, sample_rate, channels, codec}`, `Position(Duration)` mỗi ~100ms, `TrackEnded{path}`,
  `TrackStarted{path}` (khi tự chuyển sang bài đã queue), `Error(String)`, `Underrun`.
- **Trait `Sink`**: `fn spec(&self) -> (sample_rate, channels)`, nhận một `rtrb::Consumer<f32>`.
  - `CpalSink`: device mặc định, config mặc định của device (không ép 44.1k), callback chỉ `pop` từ ring,
    thiếu thì ghi 0 và đếm underrun. Không allocate, không lock trong callback.
  - `WavSink`: kéo ring ra file WAV cho đến khi nhận `Stop`. Dùng cho test, không có device.
- `DecoderThread`:
  - Giữ `current: Option<Decoded>` và `next: Option<Decoded>` (đã mở sẵn demuxer + decoder, chưa decode).
  - `FormatOptions { enable_gapless: true }` để symphonia tự cắt encoder delay/padding của MP3/AAC.
  - Vòng lặp: decode packet → convert sang f32 interleaved → nhân gain → nếu rate file ≠ rate sink thì qua rubato
    `FixedAsync` (process_into_buffer, buffer cấp sẵn) → push vào ring; ring đầy thì ngủ 2ms.
  - Hết `current`: bắn `TrackEnded`, lấy `next` làm `current` **không** flush ring, bắn `TrackStarted`. Đó là gapless.
    Vì luôn resample về rate sink nên đổi 44.1k → 96k vẫn gapless (hơn Cadence).
  - Gain = volume (tuyến tính, 0..1 đưa qua đường cong `v^2`) × ReplayGain: `10^(gain_db/20)` có kẹp bằng peak để
    không clip; gain_db lấy từ `Load` kèm `Option<f32>`; mặc định dùng track gain, preamp 0 dB.
  - Seek: dừng push, `format.seek(SeekMode::Coarse, ...)`, reset decoder, **flush ring** (drain consumer side bằng
    cờ generation: tăng `generation` atomic, sink bỏ sample thuộc generation cũ; nếu rtrb không hỗ trợ thì làm
    ring mới và đổi qua `Arc<AtomicPtr>` — implementer chọn cách đơn giản hơn và ghi lại).
  - Vị trí phát = frames đã push − số frame còn trong ring, chia rate sink; bắn `Position` mỗi 100ms.
- `CpalEngine` implement `AudioEngine` (trait từ M0, mở rộng thêm `load`, `stop`, `events() -> Receiver<Event>`).

### Test audio (không cần loa)
- `tests/gapless.rs`: `WavSink`, nạp fixture sine 440Hz rồi queue 880Hz, chạy đến `TrackEnded` thứ hai, đọc WAV ra:
  tổng số frame = tổng hai file ± 1 frame cho mỗi lần resample; **không có đoạn ≥ 2ms toàn 0 ở vùng ranh giới**
  (±50ms quanh điểm nối); cả 3 bộ FLAC→FLAC, MP3→MP3, FLAC→M4A.
- `tests/replaygain.rs`: cùng file với gain −6.02 dB thì RMS đầu ra ≈ 1/2 RMS gốc (sai số 2%).
- `tests/seek.rs`: seek tới 1.0s rồi render, số frame ≈ 1.0s còn lại.
- Resample test: fixture 48k (thêm một file ffmpeg `-ar 48000`) render qua sink 44.1k, số frame ≈ tỉ lệ.

## Example chạy tay
- `cargo run -p aurum-core --example scan -- <thư mục>`: in số bài, album, thời gian quét.
- `cargo run -p aurum-audio --example play -- a.flac b.mp3`: phát ra loa thật qua CpalSink, in Position, thoát khi hết.

## Định nghĩa xong
- `cargo test --workspace` xanh trên macOS, CI xanh cả hai job (Windows chỉ build vì runner không có audio device;
  test audio dùng WavSink nên vẫn chạy được trên Windows runner, bật trong CI).
- Example `play` phát được hai file thật trên Mac, implementer nghe không được nên xác nhận bằng log `TrackStarted`
  đúng lúc và không có `Underrun`.
- Bảng milestone cập nhật.

## Rủi ro
- API symphonia 0.6 đổi nhiều so với 0.5 (AudioBuffer, timebase, `next_packet` trả `Ok(None)`): implementer phải đọc
  source trong registry, không code từ trí nhớ.
- ReplayGain `ItemKey` trong lofty chưa kiểm chứng tên chính xác; nếu khác thì đọc `lofty/src/tag/item.rs`.
- cpal trên Mac mặc định có thể cho f32 48k; trên Windows WASAPI shared có thể là 48k hoặc 44.1k, engine không được
  giả định.

## Kết quả (2026-11-09)

- Core: 10 test tích hợp + 7 test queue xanh. Quét thử `~/Music` trên Mac dev: 326 bài, 101 album, 210ms, 1 file WAV lạ bị lofty từ chối (scan vẫn tiếp tục). Scanner bỏ qua `remove_missing` khi một root không đọc được, để ổ ngắt kết nối không xoá thư viện.
- Audio: 24 test xanh, 1 `#[ignore]`. Gapless FLAC→FLAC và MP3→MP3 đúng từng frame (176400/176400), đoạn ~0 dài nhất ở điểm nối 1 frame. Resample 48k→44.1k ra đúng 88200 frame. ReplayGain −6.02 dB cho RMS đúng 0.5000. Example `play` qua cpal thật (Mac: 48k, 2 kênh, f32) không underrun.
- Lệch plan đã chấp nhận: gapless nằm ở `AudioDecoderOptions::gapless` (symphonia 0.6 bỏ `FormatOptions::enable_gapless`); resample dùng `rubato::Fft` FixedSync vì tỉ lệ cố định theo track; gain áp sau resample kèm `GainRamp` chống click; `TrackEnded/TrackStarted` bắn theo playhead (lúc nghe thấy) chứ không phải lúc decode xong.

## Tồn đọng chuyển sang backlog

- **M4A/AAC không cắt priming/padding**: `symphonia-format-isomp4 0.6.1` parse `elst` nhưng không dùng, không đọc iTunSMPB. File AAC thừa ~1912 frame (~43ms) ở rìa, join vẫn không có đoạn lặng. Test `flac_to_m4a_exact_length` để `#[ignore]`. Hướng: tự đọc `elst`/iTunSMPB trong `decoder.rs` và cắt phía mình (M4), hoặc chờ upstream.
- Đổi volume có độ trễ ≤ 300ms (bằng ring) vì gain áp ở decoder. M2 nếu thấy UI lag thì chuyển volume vào callback qua atomic.
- Downmix nhiều kênh chỉ copy kênh trùng, chưa đúng chuẩn 5.1 → stereo.
- Chưa có test seek khi ring đã đầy (test hiện tại seek ngay sau load).
