param(
    [string]$Executable = 'C:\t\cargo-targets\woodshed\debug\redshank-desktop.exe',
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\receipts\alignment-guard'),
    [ValidateSet(8000, 48000)][int]$SampleRate = 48000
)

$ErrorActionPreference = 'Stop'
$receiptRoot = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $receiptRoot | Out-Null
$originalPath = Join-Path $receiptRoot 'changing-tones-12s.wav'
$shiftedPath = Join-Path $receiptRoot 'preroll-30s-plus-tones.wav'
$markerPath = Join-Path $receiptRoot 'fixture-owner.txt'

function Write-ReceiptWave([string]$Path, [byte[]]$Pcm) {
    $writer = [IO.BinaryWriter]::new([IO.File]::Open($Path, [IO.FileMode]::CreateNew))
    try {
        $writer.Write([Text.Encoding]::ASCII.GetBytes('RIFF'))
        $writer.Write([uint32](36 + $Pcm.Length))
        $writer.Write([Text.Encoding]::ASCII.GetBytes('WAVEfmt '))
        $writer.Write([uint32]16)
        $writer.Write([uint16]1)
        $writer.Write([uint16]1)
        $writer.Write([uint32]$SampleRate)
        $writer.Write([uint32]($SampleRate * 2))
        $writer.Write([uint16]2)
        $writer.Write([uint16]16)
        $writer.Write([Text.Encoding]::ASCII.GetBytes('data'))
        $writer.Write([uint32]$Pcm.Length)
        $writer.Write($Pcm)
    } finally { $writer.Dispose() }
}

if (-not (Test-Path -LiteralPath $originalPath) -and -not (Test-Path -LiteralPath $shiftedPath)) {
    $stream = [IO.MemoryStream]::new()
    $samples = [IO.BinaryWriter]::new($stream)
    try {
        # Deterministic nonrepeating sequence of 16 spectral bands, changing
        # every 200 ms. This is synthetic input, never a manufactured fingerprint.
        $samplesPerStep = [int]($SampleRate / 5)
        for ($sample = 0; $sample -lt (12 * $SampleRate); $sample++) {
            if ($sample % $samplesPerStep -eq 0) {
                $step = [int]($sample / $samplesPerStep)
                $hash = ([uint64]($step + 17) * [uint64]747796405) -band [uint64]4294967295
                $band = ($hash -bxor ($hash -shr 16)) % 16
                $frequency = 110.0 * [Math]::Pow(3500.0 / 110.0, $band / 15.0)
            }
            $value = [Math]::Sin(2.0 * [Math]::PI * $frequency * $sample / $SampleRate) * 6000.0
            $samples.Write([int16]$value)
        }
        $pcm = $stream.ToArray()
    } finally { $samples.Dispose(); $stream.Dispose() }
    Write-ReceiptWave $originalPath $pcm
    $prefixBytes = 30 * $SampleRate * 2
    $shifted = [byte[]]::new($prefixBytes + $pcm.Length)
    [Buffer]::BlockCopy($pcm, 0, $shifted, $prefixBytes, $pcm.Length)
    Write-ReceiptWave $shiftedPath $shifted
    Set-Content -LiteralPath $markerPath -Value 'Redshank alignment-guard generated changing-tone PCM fixtures and isolated model data; preserve logs as acceptance evidence.'
} elseif (-not (Test-Path -LiteralPath $markerPath) -or -not (Test-Path -LiteralPath $originalPath) -or -not (Test-Path -LiteralPath $shiftedPath)) {
    throw 'Incomplete or unmarked alignment fixtures; preserve existing evidence and use a dedicated output directory.'
}

$fixtureBytes = [IO.File]::ReadAllBytes($originalPath)
if ([BitConverter]::ToUInt32($fixtureBytes, 24) -ne $SampleRate) {
    throw 'Existing fixture has a different sample rate; use its recorded rate or a dedicated output directory.'
}

& (Join-Path $PSScriptRoot 'identity-guard-receipt.ps1') -Executable $Executable `
    -OutputDirectory $receiptRoot -Mode alignment-guard -InputFixture $originalPath -AlignmentFixture $shiftedPath
