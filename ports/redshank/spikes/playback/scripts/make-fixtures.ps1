param(
    [string]$OutputDir = (Join-Path $PSScriptRoot "..\target\fixtures")
)

$ErrorActionPreference = "Stop"
$ffmpeg = Get-Command ffmpeg -ErrorAction Stop
$resolvedOutput = [System.IO.Path]::GetFullPath($OutputDir)
New-Item -ItemType Directory -Force -Path $resolvedOutput | Out-Null

$source = "sine=frequency=440:sample_rate=48000:duration=60"
& $ffmpeg.Source -hide_banner -loglevel error -y -f lavfi -i $source `
    -af "volume=0.015" -ac 1 -b:a 64k (Join-Path $resolvedOutput "redshank.mp3")
& $ffmpeg.Source -hide_banner -loglevel error -y -f lavfi -i $source `
    -af "volume=0.015" -ac 1 -c:a aac -profile:a aac_low -b:a 64k `
    -movflags +faststart (Join-Path $resolvedOutput "redshank.m4a")

$episodeSource = "sine=frequency=440:sample_rate=48000:duration=900"
& $ffmpeg.Source -hide_banner -loglevel error -y -f lavfi -i $episodeSource `
    -af "volume=0.015" -ac 1 -c:a aac -profile:a aac_low -b:a 64k `
    -movflags +faststart (Join-Path $resolvedOutput "redshank-episode.m4a")

Get-ChildItem -LiteralPath $resolvedOutput -File | Select-Object Name,Length
