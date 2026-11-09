#!/usr/bin/env bash
# Regenerates the audio fixtures used by aurum-core tests. Requires ffmpeg.
# Every file is mono 44.1 kHz and under 100 KB. Run from anywhere; output lands next to this script.
set -euo pipefail
cd "$(dirname "$0")"

FF=(ffmpeg -y -loglevel error -f lavfi -ac 1)
SRC() { echo "sine=frequency=$1:sample_rate=44100:duration=${2:-2}"; }

# FLAC: album "Tones", explicit album_artist, ReplayGain tags.
for spec in "440 1" "880 2"; do
  set -- $spec
  "${FF[@]}" -i "$(SRC "$1")" -ac 1 -c:a flac -fflags +bitexact \
    -metadata title="Sine $1 FLAC" -metadata artist="Aurum Test" \
    -metadata album="Tones" -metadata album_artist="Aurum Test" \
    -metadata track="$2" -metadata date="2020" -metadata genre="Test" \
    -metadata REPLAYGAIN_TRACK_GAIN="-6.02 dB" -metadata REPLAYGAIN_TRACK_PEAK=0.5 \
    "sine$1.flac"
done

# MP3: album "Beeps", deliberately NO album_artist (fallback to artist).
for spec in "440 1" "880 2"; do
  set -- $spec
  "${FF[@]}" -i "$(SRC "$1")" -ac 1 -c:a libmp3lame -b:a 64k \
    -metadata title="Sine $1 MP3" -metadata artist="Beep Band" \
    -metadata album="Beeps" -metadata track="$2" -metadata date="2021" \
    -metadata REPLAYGAIN_TRACK_GAIN="-3.00 dB" -metadata REPLAYGAIN_TRACK_PEAK=0.8 \
    "sine$1.mp3"
done

# M4A (AAC): album_artist differs from artist, disc 2.
for spec in "440 1" "880 2"; do
  set -- $spec
  "${FF[@]}" -i "$(SRC "$1")" -ac 1 -c:a aac -b:a 48k \
    -metadata title="Sine $1 M4A" -metadata artist="Guest Artist" \
    -metadata album="Tones" -metadata album_artist="Aurum Test" \
    -metadata track="$2" -metadata disc=2 -metadata date="2022" \
    "sine$1.m4a"
done

# WAV, 1 second (keeps the file under 100 KB), no tags at all.
for f in 440 880; do
  "${FF[@]}" -i "$(SRC "$f" 1)" -ac 1 -c:a pcm_s16le -map_metadata -1 -fflags +bitexact \
    "sine$f.wav"
done
