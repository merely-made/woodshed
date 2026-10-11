$ErrorActionPreference = 'Stop'
$version = if ($env:GITHUB_REF_TYPE -eq 'tag') {
  if (-not $env:GITHUB_REF_NAME.StartsWith('woodshed-v', [StringComparison]::Ordinal)) {
    throw "Woodshed packaging requires a woodshed-v* tag."
  }
  $env:GITHUB_REF_NAME.Substring('woodshed-v'.Length)
} else {
  '0.1.0-alpha.0.manual'
}
.\scripts\package-windows.ps1 -Version $version
"version=$version" >> $env:GITHUB_OUTPUT
