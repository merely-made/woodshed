#Requires -Version 7.0
<#
Subscribes through Redshank's shipping command and preserves the listener store.
Run only after building a desktop with the scenario subscribe:<url> action.
Evidence is never overwritten; use an explicitly chosen EvidenceRoot for a rerun.
#>
param(
    [string]$Executable = 'C:\t\cargo-targets\woodshed\debug\redshank-desktop.exe',
    [string]$DataDirectory = $(if ($env:LOCALAPPDATA) { Join-Path $env:LOCALAPPDATA 'Redshank' }),
    [string]$EvidenceRoot = 'C:\Users\mark_\Code\testing\woodshed\redshank-podcasts\subscriptions',
    [ValidateRange(30, 600)][int]$TimeoutSeconds = 600
)

$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($DataDirectory)) { throw 'LOCALAPPDATA is unavailable; supply DataDirectory explicitly.' }
$dataRoot = [IO.Path]::GetFullPath($DataDirectory)
$outputRoot = [IO.Path]::GetFullPath($EvidenceRoot)
$receiptBinary = (Resolve-Path -LiteralPath $Executable).Path
if ($dataRoot -eq $outputRoot -or $dataRoot.StartsWith($outputRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
    $outputRoot.StartsWith($dataRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Keep listener data and subscription evidence in separate directories.'
}
if (Get-Process -Name 'redshank-desktop' -ErrorAction SilentlyContinue) {
    throw 'An existing Redshank process may own the listener store. Close it before running; this script will not stop it.'
}
$marker = Join-Path $outputRoot 'receipt-owner.txt'
$markerText = 'Redshank public podcast subscription receipt; listener data is separate and must be preserved.'
if (Test-Path -LiteralPath $outputRoot) {
    if (-not (Test-Path -LiteralPath $marker) -or (Get-Content -LiteralPath $marker -Raw).Trim() -ne $markerText) {
        throw 'Existing evidence directory is not owned by this runner.'
    }
    if (Get-ChildItem -LiteralPath $outputRoot -Force | Where-Object Name -ne 'receipt-owner.txt') {
        throw 'Existing evidence is preserved. Choose an explicit different EvidenceRoot for another run.'
    }
}
$feeds = @(
    @{ name = 'Bad Faith'; url = 'https://badfaith.libsyn.com/rss'; title = 'Bad Faith'; accepted = @('https://badfaith.libsyn.com/rss', 'https://rss.libsyn.com/shows/293411/destinations/2303135.xml') },
    @{ name = 'TrueAnon'; url = 'https://www.patreon.com/public-rss/2963533?show=875184'; title = 'TRUE ANON TRUTH FEED'; accepted = @('https://www.patreon.com/public-rss/2963533?show=875184') },
    @{ name = 'House of Bob'; url = 'https://feeds.megaphone.fm/houseofbob'; title = 'House of Bob'; accepted = @('https://feeds.megaphone.fm/houseofbob') },
    @{ name = 'Decoder'; url = 'https://feeds.megaphone.fm/recodedecode'; title = 'Decoder with Nilay Patel'; accepted = @('https://feeds.megaphone.fm/recodedecode') },
    @{ name = 'Regulation Podcast'; url = 'https://feeds.megaphone.fm/fface'; title = 'Regulation Podcast'; accepted = @('https://feeds.megaphone.fm/fface') }
)

function Get-PublishedGenerations {
    if (Test-Path -LiteralPath $dataRoot) {
        Get-ChildItem -LiteralPath $dataRoot -File | Where-Object Name -Match '^state-\d+\.json$' |
            Sort-Object { [uint64]($_.BaseName.Substring(6)) } -Descending
    }
}

function Read-LatestModel {
    foreach ($file in Get-PublishedGenerations) {
        try { $model = Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json -AsHashtable }
        catch { continue }
        # Match required model containers before accepting JSON as a generation.
        # The native store remains the authority for full Rust deserialization.
        if ($model -isnot [Collections.IDictionary] -or $model.schema_version -isnot [long] -and $model.schema_version -isnot [int]) { continue }
        $valid = $true
        foreach ($field in @('library', 'progress', 'annotations', 'settings')) {
            if ($model[$field] -isnot [Collections.IDictionary]) { $valid = $false }
        }
        if (-not $model.Contains('queue') -or $model.queue -isnot [array]) { $valid = $false }
        if ($model.Contains('subscriptions') -and $model.subscriptions -isnot [Collections.IDictionary]) { $valid = $false }
        if ($valid) { return @{ path = $file.FullName; model = $model } }
    }
    return $null
}

function Get-ModelCounts($Snapshot) {
    if ($null -eq $Snapshot) { return $null }
    $counts = [ordered]@{}
    foreach ($field in @('subscriptions', 'library', 'queue', 'progress', 'annotations', 'transcripts')) {
        $counts[$field] = @($Snapshot.model[$field]).Count
        if ($Snapshot.model[$field] -is [Collections.IDictionary]) { $counts[$field] = $Snapshot.model[$field].Count }
        if ($null -eq $Snapshot.model[$field]) { $counts[$field] = 0 }
    }
    return $counts
}

$before = Read-LatestModel
$originalGenerations = @{}
foreach ($file in Get-PublishedGenerations) { $originalGenerations[$file.FullName] = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash }
if ($originalGenerations.Count -gt 0 -and $null -eq $before) { throw 'No readable existing model generation; refusing to start a fresh listener model over it.' }
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
Set-Content -LiteralPath $marker -Value $markerText
$scenarioPath = Join-Path $outputRoot 'subscribe-podcasts.scn'
$scenario = [Collections.Generic.List[string]]::new()
$scenario.Add('# Public feeds only. No media download, playback or transcript download commands.')
$scenario.Add('wait 18000')
$scenario.Add('act tab:library')
$scenario.Add('settle 3')
foreach ($feed in $feeds) {
    $scenario.Add('act subscribe:' + $feed.url)
    $scenario.Add('wait 18000')
    $scenario.Add('assert text ' + $feed.title)
}
$scenario.Add('assert snap feed-count >= 5')
$scenario.Add('capture subscribed-podcasts')
$scenario.Add('log public podcast subscriptions complete')
Set-Content -LiteralPath $scenarioPath -Value $scenario
$savedEnvironment = @{}
foreach ($name in @('REDSHANK_HEADED_RECEIPT', 'REDSHANK_RECEIPT', 'REDSHANK_SCENARIO', 'REDSHANK_CAPTURE_DIR', 'REDSHANK_DATA_DIR', 'REDSHANK_WIDTH', 'REDSHANK_HEIGHT')) {
    $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
$binaryHash = (Get-FileHash -LiteralPath $receiptBinary -Algorithm SHA256).Hash
$started = [DateTime]::UtcNow
$appProcess = $null
$failure = $null
$exitCode = $null
$after = $null
$subscriptions = @()
try {
    Remove-Item Env:REDSHANK_HEADED_RECEIPT -ErrorAction SilentlyContinue
    Remove-Item Env:REDSHANK_RECEIPT -ErrorAction SilentlyContinue
    $env:REDSHANK_SCENARIO = $scenarioPath
    $env:REDSHANK_CAPTURE_DIR = $outputRoot
    $env:REDSHANK_DATA_DIR = $dataRoot
    $env:REDSHANK_WIDTH = '1280'
    $env:REDSHANK_HEIGHT = '900'
    $appProcess = Start-Process -FilePath $receiptBinary -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $outputRoot 'stdout.log') -RedirectStandardError (Join-Path $outputRoot 'stderr.log')
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while (-not $appProcess.WaitForExit(500)) {
        if ([DateTime]::UtcNow -ge $deadline) { throw 'Owned subscription process exceeded its deadline.' }
    }
    $appProcess.Refresh()
    $exitCode = $appProcess.ExitCode
    if ($exitCode -ne 0) { throw "Subscription process exited $exitCode." }
    $sentinel = Join-Path $outputRoot 'scenario.done'
    if (-not (Test-Path -LiteralPath $sentinel) -or (Get-Content -LiteralPath $sentinel -TotalCount 1) -ne 'RESULT ok') { throw 'Scenario did not record RESULT ok.' }
    $png = [IO.File]::ReadAllBytes((Join-Path $outputRoot 'subscribed-podcasts.png'))
    if ($png.Length -lt 24 -or [BitConverter]::ToString($png, 0, 8) -ne '89-50-4E-47-0D-0A-1A-0A') { throw 'Subscription capture is missing or invalid.' }
    $after = Read-LatestModel
    if ($null -eq $after) { throw 'No valid published listener model after subscription.' }
    foreach ($entry in $originalGenerations.GetEnumerator()) {
        if (-not (Test-Path -LiteralPath $entry.Key) -or (Get-FileHash -LiteralPath $entry.Key -Algorithm SHA256).Hash -ne $entry.Value) { throw 'An original model generation changed or disappeared.' }
    }
    if ($null -ne $before) {
        foreach ($field in @('library', 'subscriptions', 'annotations', 'transcripts')) {
            if ($before.model[$field] -is [Collections.IDictionary]) {
                foreach ($key in $before.model[$field].Keys) {
                    if (-not $after.model[$field].Contains($key)) { throw "Existing $field entry disappeared during subscription." }
                }
            }
        }
    }
    foreach ($feed in $feeds) {
        $matches = @($after.model.subscriptions.Values | Where-Object { $_.feed_url -in $feed.accepted })
        if ($matches.Count -eq 0) { throw ('Requested subscription was not persisted: ' + $feed.name) }
        $stored = $matches[0]
        if ($stored.last_refreshed_ms -lt ([DateTimeOffset]$started).ToUnixTimeMilliseconds()) {
            throw ('Requested subscription was not refreshed during this run: ' + $feed.name)
        }
        $episodes = @($after.model.library.Values | Where-Object { $_.kind -eq 'feed_episode' -and $_.feed_url -eq $stored.feed_url })
        if ($episodes.Count -eq 0) { throw ('Requested subscription has no imported episodes: ' + $feed.name) }
        $advertised = @($episodes | Where-Object { $_.facts.transcripts.Count -gt 0 })
        $types = @($episodes | ForEach-Object { $_.facts.transcripts } | ForEach-Object { $_.media_type } | Where-Object { $_ } | Sort-Object -Unique)
        $subscriptions += [ordered]@{
            name = $feed.name; requested_url = $feed.url; stored_url = $stored.feed_url
            episode_count = $episodes.Count; episodes_with_advertised_transcripts = $advertised.Count
            transcript_media_types = $types; last_refreshed_ms = $stored.last_refreshed_ms
        }
    }
    if ((Get-FileHash -LiteralPath $receiptBinary -Algorithm SHA256).Hash -ne $binaryHash) { throw 'Executable changed during subscription.' }
} catch {
    $failure = $_.Exception.Message
} finally {
    if ($null -ne $appProcess -and -not $appProcess.HasExited) {
        $appProcess.Kill()
        if (-not $appProcess.WaitForExit(10000)) { $failure = 'Owned process did not exit after its deadline and stop request.' }
    }
    foreach ($entry in $savedEnvironment.GetEnumerator()) {
        if ($null -eq $entry.Value) { Remove-Item -LiteralPath ('Env:' + $entry.Key) -ErrorAction SilentlyContinue }
        else { [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value, 'Process') }
    }
}
[ordered]@{
    passed = $null -eq $failure; failure = $failure; exit_code = $exitCode
    started_utc = $started.ToString('o'); finished_utc = [DateTime]::UtcNow.ToString('o')
    binary = $receiptBinary; binary_sha256 = $binaryHash; data_directory = $dataRoot
    scenario = $scenarioPath; scenario_sha256 = (Get-FileHash -LiteralPath $scenarioPath -Algorithm SHA256).Hash
    previous_generation = $(if ($before) { $before.path }); model_generation = $(if ($after) { $after.path })
    model_sha256 = $(if ($after) { (Get-FileHash -LiteralPath $after.path -Algorithm SHA256).Hash })
    previous_counts = (Get-ModelCounts $before); model_counts = (Get-ModelCounts $after)
    preserved_original_generations = $originalGenerations.Count; subscriptions = $subscriptions
    evidence = 'Native scenario commands, DOM assertions, presented-frame capture and read-only durable-model inspection. Reports public requested feed metadata and aggregate counts; the listener model is neither seeded nor copied into evidence.'
} | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $outputRoot 'summary.json')
if ($failure) { throw "Podcast subscription failed: $failure Evidence preserved at $outputRoot" }
Get-Content -LiteralPath (Join-Path $outputRoot 'summary.json')
