param(
    [string]$Executable = 'C:\t\cargo-targets\woodshed\debug\redshank-desktop.exe',
    [string]$OutputDirectory = '',
    [ValidateSet('identity-guard', 'alignment-guard')][string]$Mode = 'identity-guard',
    [string]$InputFixture = '',
    [string]$AlignmentFixture = ''
)

$ErrorActionPreference = 'Stop'
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $PSScriptRoot ('..\receipts\' + $Mode) }
$receiptRoot = [IO.Path]::GetFullPath($OutputDirectory)
$receiptBinary = (Resolve-Path -LiteralPath $Executable).Path
New-Item -ItemType Directory -Force -Path $receiptRoot | Out-Null
$fixturePath = if ($InputFixture) { (Resolve-Path -LiteralPath $InputFixture).Path } else { Join-Path $receiptRoot 'silence-10s.wav' }
if ($Mode -eq 'alignment-guard' -and (-not $InputFixture -or -not $AlignmentFixture)) {
    throw 'Use alignment-guard-receipt.ps1 to prepare the two synthetic audio inputs.'
}
$dataPath = Join-Path $receiptRoot 'data'
$stdoutPath = Join-Path $receiptRoot 'stdout.log'
$stderrPath = Join-Path $receiptRoot 'stderr.log'
$summaryPath = Join-Path $receiptRoot 'summary.json'
$markerPath = Join-Path $receiptRoot 'fixture-owner.txt'

# This fixture is ten seconds of 48 kHz mono 16-bit PCM silence. It exercises
# the real decoder/output/seek path without requiring a downloaded recording.
if (-not (Test-Path -LiteralPath $fixturePath)) {
    $samplesBytes = 48000 * 10 * 2
    $writer = [IO.BinaryWriter]::new([IO.File]::Open($fixturePath, [IO.FileMode]::CreateNew))
    try {
        $writer.Write([Text.Encoding]::ASCII.GetBytes('RIFF'))
        $writer.Write([uint32](36 + $samplesBytes))
        $writer.Write([Text.Encoding]::ASCII.GetBytes('WAVEfmt '))
        $writer.Write([uint32]16)
        $writer.Write([uint16]1)
        $writer.Write([uint16]1)
        $writer.Write([uint32]48000)
        $writer.Write([uint32]96000)
        $writer.Write([uint16]2)
        $writer.Write([uint16]16)
        $writer.Write([Text.Encoding]::ASCII.GetBytes('data'))
        $writer.Write([uint32]$samplesBytes)
        $writer.Write([byte[]]::new($samplesBytes))
    } finally {
        $writer.Dispose()
    }
    Set-Content -LiteralPath $markerPath -Value 'Redshank identity-guard generated PCM fixture and isolated model data; preserve logs as acceptance evidence.'
} elseif (-not (Test-Path -LiteralPath $markerPath)) {
    throw 'The existing fixture has no ownership marker; choose a dedicated receipt output directory.'
}

$oldMode = [Environment]::GetEnvironmentVariable('REDSHANK_HEADED_RECEIPT', 'Process')
$oldData = [Environment]::GetEnvironmentVariable('REDSHANK_DATA_DIR', 'Process')
$oldScenario = [Environment]::GetEnvironmentVariable('REDSHANK_SCENARIO', 'Process')
$oldAlignment = [Environment]::GetEnvironmentVariable('REDSHANK_ALIGNMENT_FIXTURE', 'Process')
$started = [DateTime]::UtcNow
try {
    $env:REDSHANK_HEADED_RECEIPT = $Mode
    $env:REDSHANK_DATA_DIR = $dataPath
    Remove-Item Env:REDSHANK_SCENARIO -ErrorAction SilentlyContinue
    if ($AlignmentFixture) { $env:REDSHANK_ALIGNMENT_FIXTURE = (Resolve-Path -LiteralPath $AlignmentFixture).Path }
    $receiptProcess = Start-Process -FilePath $receiptBinary -ArgumentList ('"' + $fixturePath + '"') `
        -WindowStyle Hidden -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath -PassThru
    if (-not $receiptProcess.WaitForExit(60000)) {
        $receiptProcess.Kill()
        $receiptProcess.WaitForExit()
        throw 'Native receipt process exceeded its 60-second outer deadline.'
    }
    $receiptProcess.Refresh()
    $exitCode = $receiptProcess.ExitCode
    $stdout = Get-Content -LiteralPath $stdoutPath -Raw
    $passed = $exitCode -eq 0 -and $stdout -match ('redshank-headed-receipt ' + $Mode + ' PASS')
    $receiptSummary = [ordered]@{
        mode = $Mode
        passed = $passed
        exit_code = $exitCode
        started_utc = $started.ToString('o')
        finished_utc = [DateTime]::UtcNow.ToString('o')
        binary = $receiptBinary
        binary_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $receiptBinary).Hash
        fixture_path = $fixturePath
        fixture_sample_rate_hz = [BitConverter]::ToUInt32([IO.File]::ReadAllBytes($fixturePath), 24)
        fixture_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $fixturePath).Hash
        evidence = 'Automated native winit/Genet window with ordinary Desktop commands, projection, decoder and output runtime. No pointer/keyboard or human-observed visual/acoustic acceptance.'
        cases = if ($Mode -eq 'alignment-guard') { @('actual pre-rate fingerprint capture at 2x with reaction offset', 'digest-bound cached search finds inserted 30-second preroll', 'derived position acknowledged seek', 'captured original target preserved') } else { @('same-copy acknowledged seek', 'different/unproven open and span refusal before seeking', 'paused position unchanged after refusals', 'explicit approximate acknowledged seek', 'original note targets preserved') }
        retained_inputs = @([IO.Path]::GetFileName($fixturePath), 'data/')
    }
    if ($AlignmentFixture) {
        $receiptSummary.alignment_fixture_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $AlignmentFixture).Hash
        $receiptSummary.alignment_fixture_path = [IO.Path]::GetFullPath($AlignmentFixture)
        $receiptSummary.retained_inputs += [IO.Path]::GetFileName($AlignmentFixture)
        $receiptSummary.evidence += ' Synthetic changing tones only; no real-podcast, codec transformation, or confidence calibration claim.'
    }
    $receiptSummary | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $summaryPath
    Write-Output $stdout
    if (-not $passed) {
        throw "$Mode receipt failed; see $stdoutPath and $stderrPath. Inputs and model evidence are preserved."
    }
} finally {
    foreach ($entry in @{
        REDSHANK_HEADED_RECEIPT = $oldMode
        REDSHANK_DATA_DIR = $oldData
        REDSHANK_SCENARIO = $oldScenario
        REDSHANK_ALIGNMENT_FIXTURE = $oldAlignment
    }.GetEnumerator()) {
        if ($null -eq $entry.Value) {
            Remove-Item -LiteralPath ("Env:" + $entry.Key) -ErrorAction SilentlyContinue
        } else {
            [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value, 'Process')
        }
    }
}
