param(
    [ValidateSet('lifecycle', 'transcript')][string]$Case = 'lifecycle',
    [string]$Executable = 'C:\t\cargo-targets\woodshed\debug\redshank-desktop.exe',
    [string]$ReceiptRoot = 'C:\Users\mark_\Code\testing\woodshed\redshank-mesquite',
    [ValidateRange(10, 300)][int]$TimeoutSeconds = 120
)

$ErrorActionPreference = 'Stop'
$portRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$outputRoot = [IO.Path]::GetFullPath((Join-Path $ReceiptRoot $Case))
$dataRoot = [IO.Path]::GetFullPath((Join-Path $outputRoot 'data'))
if (-not $dataRoot.StartsWith($outputRoot + [IO.Path]::DirectorySeparatorChar)) { throw 'Receipt data escaped its owner directory.' }
$marker = Join-Path $outputRoot 'receipt-owner.txt'
$markerText = 'Redshank Mesquite lifecycle receipt: isolated model, captures, logs and owned local fixture processes.'
if (Test-Path -LiteralPath $outputRoot) {
    if (-not (Test-Path -LiteralPath $marker) -or (Get-Content -LiteralPath $marker -Raw).Trim() -ne $markerText) {
        throw 'Existing receipt directory is not owned by this runner.'
    }
    if (Get-ChildItem -LiteralPath $outputRoot -Force | Where-Object Name -ne 'receipt-owner.txt') {
        throw 'Existing receipt evidence is preserved. Archive it explicitly before reusing this stable case path.'
    }
}
$receiptBinary = (Resolve-Path -LiteralPath $Executable).Path
$scenarioName = if ($Case -eq 'lifecycle') { 'mesquite_lifecycle' } else { 'mesquite_transcript_wait' }
$fixtureName = 'empty'
$scenarioPath = Join-Path $portRoot ('scenarios/' + $scenarioName + '.scn')
$fixturePath = Join-Path $portRoot ('scenarios/fixtures/' + $fixtureName)
$expectedCaptures = if ($Case -eq 'lifecycle') {
    @('lifecycle-listen.png', 'lifecycle-selector-notes.png', 'lifecycle-final.png')
} else { @('transcript-before-fetch.png', 'transcript-after-quiescence.png', 'transcript-cue-seek.png') }
$httpProcesses = [Collections.Generic.List[Diagnostics.Process]]::new()
$appProcess = $null
$savedEnvironment = @{}
foreach ($name in @('REDSHANK_HEADED_RECEIPT', 'REDSHANK_RECEIPT', 'REDSHANK_SCENARIO', 'REDSHANK_CAPTURE_DIR', 'REDSHANK_DATA_DIR', 'REDSHANK_WIDTH', 'REDSHANK_HEIGHT')) {
    $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}

function Test-ReceiptPort([int]$Port) {
    $client = [Net.Sockets.TcpClient]::new()
    try {
        $pending = $client.ConnectAsync('127.0.0.1', $Port)
        return $pending.Wait(200) -and $client.Connected
    } catch { return $false } finally { $client.Dispose() }
}

New-Item -ItemType Directory -Path $dataRoot -Force | Out-Null
Set-Content -LiteralPath $marker -Value $markerText
foreach ($file in Get-ChildItem -LiteralPath $fixturePath -Force) {
    Copy-Item -LiteralPath $file.FullName -Destination $dataRoot -Recurse
}
$started = [DateTime]::UtcNow
$binaryHash = (Get-FileHash -LiteralPath $receiptBinary -Algorithm SHA256).Hash
$failure = $null
$exitCode = $null
$persistedTranscriptGeneration = $null
$offlineVerified = $false
try {
    if ($Case -eq 'transcript') {
        foreach ($port in @(8766)) {
            if (Test-ReceiptPort $port) { throw "Port $port belongs to an existing process; this runner will not reuse or stop it." }
        }
        # Build only receipt-owned fixture data from the committed empty model.
        # Nothing depends on the separate, uncommitted real-transcript lane.
        $fixtureHttpRoot = Join-Path $outputRoot 'http-fixtures'
        New-Item -ItemType Directory -Path $fixtureHttpRoot | Out-Null
        $audioPath = Join-Path $fixtureHttpRoot 'fixture.wav'
        $writer = [IO.BinaryWriter]::new([IO.File]::Open($audioPath, [IO.FileMode]::CreateNew))
        try {
            $pcmBytes = 48000 * 10 * 2
            $writer.Write([Text.Encoding]::ASCII.GetBytes('RIFF'))
            $writer.Write([uint32](36 + $pcmBytes))
            $writer.Write([Text.Encoding]::ASCII.GetBytes('WAVEfmt '))
            $writer.Write([uint32]16); $writer.Write([uint16]1); $writer.Write([uint16]1)
            $writer.Write([uint32]48000); $writer.Write([uint32]96000)
            $writer.Write([uint16]2); $writer.Write([uint16]16)
            $writer.Write([Text.Encoding]::ASCII.GetBytes('data'))
            $writer.Write([uint32]$pcmBytes); $writer.Write([byte[]]::new($pcmBytes))
        } finally { $writer.Dispose() }
        $vtt = "WEBVTT`n`n00:00.000 --> 00:05.000`nMesquite fixture first sentence.`n`n00:05.000 --> 00:10.000`nMesquite fixture second sentence.`n"
        [IO.File]::WriteAllText((Join-Path $fixtureHttpRoot 'fixture.vtt'), $vtt)
        $generation = Join-Path $dataRoot 'state-00000000000000000001.json'
        $model = Get-Content -LiteralPath $generation -Raw | ConvertFrom-Json -AsHashtable
        $model.library = @{
            'mesquite-transcript' = @{
                kind = 'feed_episode'; id = 'mesquite-transcript'; guid = 'mesquite-transcript'
                title = 'Mesquite transcript fixture'; feed_url = 'http://127.0.0.1:8766/fixture-feed'
                source = @{ kind = 'local'; path = $audioPath }
                facts = @{
                    published = $null; summary = $null; duration = '10'; artwork = $null
                    enclosure_media_type = 'audio/wav'; enclosure_byte_length = 960044; chapters = @()
                    transcripts = @(@{ url = 'http://127.0.0.1:8766/fixture.vtt'; media_type = 'text/vtt'; language = $null; relation = 'captions' })
                }
            }
        }
        $model.queue = @('mesquite-transcript')
        $model.selected_item = 'mesquite-transcript'
        $model | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $generation
        $python = (Get-Command python -ErrorAction Stop).Source
        $vttScript = Join-Path $PSScriptRoot 'transcript-receipt-server.py'
        $httpProcesses.Add((Start-Process -FilePath $python -ArgumentList @(('"' + $vttScript + '"'), ('"' + $fixtureHttpRoot + '"'), '--port', '8766', '--delay-ms', '2000') -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $outputRoot 'transcript-stdout.log') -RedirectStandardError (Join-Path $outputRoot 'transcript-stderr.log')))
        $readyDeadline = [DateTime]::UtcNow.AddSeconds(10)
        while (-not (Test-ReceiptPort 8766)) {
            if ([DateTime]::UtcNow -ge $readyDeadline -or ($httpProcesses | Where-Object HasExited)) { throw 'Owned local fixture servers did not become ready.' }
            Start-Sleep -Milliseconds 100
        }
    }
    Remove-Item Env:REDSHANK_HEADED_RECEIPT -ErrorAction SilentlyContinue
    Remove-Item Env:REDSHANK_RECEIPT -ErrorAction SilentlyContinue
    $env:REDSHANK_SCENARIO = $scenarioPath
    $env:REDSHANK_CAPTURE_DIR = $outputRoot
    $env:REDSHANK_DATA_DIR = $dataRoot
    $env:REDSHANK_WIDTH = '960'
    $env:REDSHANK_HEIGHT = '640'
    $appProcess = Start-Process -FilePath $receiptBinary -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $outputRoot 'stdout.log') -RedirectStandardError (Join-Path $outputRoot 'stderr.log')
    if (-not $appProcess.WaitForExit($TimeoutSeconds * 1000)) { throw 'Owned scenario process exceeded its receipt deadline.' }
    $appProcess.Refresh()
    $exitCode = $appProcess.ExitCode
    if ($exitCode -ne 0) { throw "Scenario process exited $exitCode." }
    $sentinel = Join-Path $outputRoot 'scenario.done'
    if (-not (Test-Path -LiteralPath $sentinel) -or (Get-Content -LiteralPath $sentinel -TotalCount 1) -ne 'RESULT ok') { throw 'Scenario did not record RESULT ok.' }
    foreach ($name in $expectedCaptures) {
        $png = [IO.File]::ReadAllBytes((Join-Path $outputRoot $name))
        if ($png.Length -lt 24 -or [BitConverter]::ToString($png, 0, 8) -ne '89-50-4E-47-0D-0A-1A-0A') { throw "Missing or invalid capture: $name" }
    }
    if ($Case -eq 'transcript') {
        # Published generations end in .json; .json.pending is not durable.
        $latest = Get-ChildItem -LiteralPath $dataRoot -File |
            Where-Object Name -Match '^state-\d{20}\.json$' |
            Sort-Object Name -Descending | Select-Object -First 1
        if ($null -eq $latest -or $latest.Name -eq 'state-00000000000000000001.json') {
            throw 'Transcript fetch did not publish a new model generation before process exit.'
        }
        $persistedModel = Get-Content -LiteralPath $latest.FullName -Raw | ConvertFrom-Json -AsHashtable
        $savedTranscript = $persistedModel.transcripts['mesquite-transcript']
        if ($null -eq $savedTranscript -or $savedTranscript.source -cne $vtt -or
            $savedTranscript.resource.url -ne 'http://127.0.0.1:8766/fixture.vtt' -or
            $savedTranscript.final_url -ne 'http://127.0.0.1:8766/fixture.vtt' -or
            $savedTranscript.retrieved_at_ms -le 0) {
            throw 'Latest published model does not contain the expected saved synthetic transcript and fetch receipt.'
        }
        $persistedTranscriptGeneration = $latest.FullName
        # Reopen the published model after stopping our transcript origin.
        foreach ($process in $httpProcesses) { if (-not $process.HasExited) { $process.Kill(); if (-not $process.WaitForExit(10000)) { throw 'Owned transcript origin did not stop.' } } }
        $offlineDeadline = [DateTime]::UtcNow.AddSeconds(3)
        while (Test-ReceiptPort 8766) {
            if ([DateTime]::UtcNow -ge $offlineDeadline) { throw 'Transcript origin is still reachable before offline restart.' }
            Start-Sleep -Milliseconds 50
        }
        $offlineRoot = Join-Path $outputRoot 'offline'
        New-Item -ItemType Directory -Path $offlineRoot | Out-Null
        $env:REDSHANK_CAPTURE_DIR = $offlineRoot
        $env:REDSHANK_SCENARIO = Join-Path $portRoot 'scenarios/mesquite_transcript_offline.scn'
        $appProcess = Start-Process -FilePath $receiptBinary -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $offlineRoot 'stdout.log') -RedirectStandardError (Join-Path $offlineRoot 'stderr.log')
        if (-not $appProcess.WaitForExit($TimeoutSeconds * 1000)) { throw 'Owned offline restart exceeded its receipt deadline.' }
        $appProcess.Refresh()
        if ($appProcess.ExitCode -ne 0 -or -not (Test-Path -LiteralPath (Join-Path $offlineRoot 'scenario.done')) -or
            (Get-Content -LiteralPath (Join-Path $offlineRoot 'scenario.done') -TotalCount 1) -ne 'RESULT ok') {
            throw 'Offline restart did not record RESULT ok.'
        }
        $offlineVerified = $true
    }
    if ((Get-FileHash -LiteralPath $receiptBinary -Algorithm SHA256).Hash -ne $binaryHash) { throw 'Executable changed during this receipt.' }
} catch {
    $failure = $_.Exception.Message
} finally {
    if ($null -ne $appProcess -and -not $appProcess.HasExited) { $appProcess.Kill(); if (-not $appProcess.WaitForExit(10000)) { $failure = 'Owned app did not exit after its stop request.' } }
    foreach ($process in $httpProcesses) { if (-not $process.HasExited) { $process.Kill(); if (-not $process.WaitForExit(10000)) { $failure = 'Owned fixture process did not exit after its stop request.' } } }
    foreach ($entry in $savedEnvironment.GetEnumerator()) {
        if ($null -eq $entry.Value) { Remove-Item -LiteralPath ('Env:' + $entry.Key) -ErrorAction SilentlyContinue }
        else { [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value, 'Process') }
    }
}
[ordered]@{
    case = $Case; passed = $null -eq $failure; failure = $failure; exit_code = $exitCode
    started_utc = $started.ToString('o'); finished_utc = [DateTime]::UtcNow.ToString('o')
    binary = $receiptBinary; binary_sha256 = $binaryHash
    scenario = $scenarioPath; scenario_sha256 = (Get-FileHash -LiteralPath $scenarioPath -Algorithm SHA256).Hash
    fixture_source = $fixturePath; isolated_data = $dataRoot; expected_captures = $expectedCaptures
    persisted_transcript_generation = $persistedTranscriptGeneration; offline_restart_verified = $offlineVerified
    evidence = 'Native presented-frame readback captures, scenario assertions and semantic role/name button and tab routing. This does not exercise clipped-target scrolling. Transcript case generates local PCM and VTT from the committed empty model; only the VTT uses an owned loopback server with a delayed response; restart verifies saved cue seeking after that origin stops.'
} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $outputRoot 'summary.json')
if ($failure) { throw "$Case receipt failed: $failure Evidence preserved at $outputRoot" }
Get-Content -LiteralPath (Join-Path $outputRoot 'scenario.done')
