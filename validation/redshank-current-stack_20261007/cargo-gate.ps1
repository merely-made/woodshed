param(
    [ValidateSet('resolve','metadata-windows','metadata-web','check-desktop','test-port','check-web','test-ipc')][string] $Gate,
    [ValidatePattern('^[a-z0-9-]+$')][string] $Label = ''
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$manifest = Join-Path $repo 'ports/redshank/Cargo.toml'
$env:CARGO_TARGET_DIR = 'C:/t/cargo-targets/woodshed'
$env:CARGO_BUILD_JOBS = '1'
$env:CARGO_PROFILE_DEV_DEBUG = '0'
$env:CARGO_PROFILE_TEST_DEBUG = '0'
$env:LIBCLANG_PATH = 'C:/Program Files/LLVM/bin'
$env:PATH = 'C:/Program Files/LLVM/bin;' + $env:PATH
$vcvars = 'C:/Program Files/Microsoft Visual Studio/2022/Community/VC/Auxiliary/Build/vcvars64.bat'
$toolEnvironment = & cmd.exe /d /s /c ('call "' + $vcvars + '" >nul && set')
if ($LASTEXITCODE -ne 0) { throw 'VS environment initialization failed' }
foreach ($line in $toolEnvironment) {
    if ($line -match '^([^=]+)=(.*)$') { [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], 'Process') }
}
$arguments = switch ($Gate) {
    'resolve' { @('+1.97.1','metadata','--manifest-path',$manifest,'--format-version','1') }
    'metadata-windows' { @('+1.97.1','metadata','--locked','--manifest-path',$manifest,'--format-version','1','--filter-platform','x86_64-pc-windows-msvc') }
    'metadata-web' { @('+1.97.1','metadata','--locked','--manifest-path',$manifest,'--format-version','1','--filter-platform','wasm32-unknown-unknown') }
    'check-desktop' { @('+1.97.1','check','--locked','--manifest-path',$manifest,'-p','redshank-desktop','-p','redshank-playback','-p','redshank-surfaces','--all-targets','-j','1') }
    'test-port' { @('+1.97.1','test','--locked','--manifest-path',$manifest,'-p','redshank-desktop','-p','redshank-playback','-p','redshank-surfaces','-p','redshank-feed','-j','1','--','--test-threads=1') }
    'check-web' { @('+1.97.1','check','--locked','--manifest-path',$manifest,'-p','redshank-web','--target','wasm32-unknown-unknown','-j','1') }
    'test-ipc' { @('+1.97.1','test','--locked','--manifest-path',$manifest,'-p','redshank-playback','--test','ipc_consumer','-j','1','--','--test-threads=1') }
}
$name = if ($Label) { $Label } else { $Gate }
$stdout = Join-Path $PSScriptRoot ($name + '.stdout.log')
$stderr = Join-Path $PSScriptRoot ($name + '.stderr.log')
$result = Join-Path $PSScriptRoot ($name + '.result.json')
foreach ($path in @($stdout,$stderr,$result)) { if (Test-Path -LiteralPath $path) { throw "Refusing existing output: $path" } }
$start = [DateTime]::UtcNow
$process = Start-Process -FilePath (Get-Command cargo.exe).Source -ArgumentList $arguments -WorkingDirectory 'C:/t' -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
$null = $process.Handle
try { $process.PriorityClass = 'BelowNormal' } catch { if (-not $process.HasExited) { throw } }
Write-Output "Started $Gate Cargo PID $($process.Id), one job, BelowNormal"
$process.WaitForExit()
if ($null -eq $process.ExitCode) { throw 'Native Cargo exit unavailable; gate cannot pass' }
@{gate=$Gate; command=@('cargo')+$arguments; cwd='C:/t'; pid=$process.Id; started_utc=$start.ToString('o'); finished_utc=[DateTime]::UtcNow.ToString('o'); exit_code=$process.ExitCode; target=$env:CARGO_TARGET_DIR; jobs=1; priority='BelowNormal'; dev_debug=0; test_debug=0; libclang_path=$env:LIBCLANG_PATH} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $result -Encoding utf8
Get-Content -LiteralPath $stderr -Tail 25 -Encoding utf8
exit $process.ExitCode
