#!/usr/bin/env bash
# Regenerates the audio fixtures used by the aurum-audio tests.
# Requires ffmpeg with libmp3lame and the native aac encoder.
# All files are mono, 2 seconds, so each stays far below 100 KB.
set -euo pipefail
cd "$(dirname "$0")"

FFMPEG="${FFMPEG:-ffmpeg}"

sine() { # freq rate
  echo "sine=frequency=$1:sample_rate=$2:duration=2"
}

for f in 440 880; do
  "$FFMPEG" -y -v error -f lavfi -i "$(sine $f 44100)" -ac 1 -c:a flac "sine${f}.flac"
  "$FFMPEG" -y -v error -f lavfi -i "$(sine $f 44100)" -ac 1 -c:a libmp3lame -b:a 96k "sine${f}.mp3"
  "$FFMPEG" -y -v error -f lavfi -i "$(sine $f 44100)" -ac 1 -c:a aac -b:a 96k "sine${f}.m4a"
done

# 48 kHz source for the resample test.
"$FFMPEG" -y -v error -f lavfi -i "$(sine 880 48000)" -ac 1 -ar 48000 -c:a flac "sine880_48k.flac"

ls -l ./*.flac ./*.mp3 ./*.m4a
