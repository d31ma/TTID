# Regression checks for the fail-closed checksum logic in scripts/install.ps1.
#
#   pwsh test/install.ps1
#
# The mirror of test/install.sh. Invoke-WebRequest has no file:// support, so
# the harness shadows it with a function that serves a fake release from a temp
# directory -- no network, no listener. TTID_BASE_URL redirects the fetch and
# only the install directory is rewritten, so the verification block under test
# is the real one. Every failure case here installed an unverified binary before
# the checks were made to fail closed.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$src = if ($args.Count -ge 1) { $args[0] } else { Join-Path $root 'scripts/install.ps1' }

$asset = 'ttid-windows-x64.exe'
$pinnedTag = 'v26.28.02'

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("ttid-install-" + [guid]::NewGuid())
$releases = Join-Path $work 'releases'
$latest = Join-Path $releases 'latest/download'
$pinned = Join-Path $releases "download/$pinnedTag"
$dest = Join-Path $work 'bin'
New-Item -ItemType Directory -Force -Path $latest, $pinned, $dest | Out-Null

function Write-Sums($dir) {
    $file = Join-Path $dir $asset
    $hash = (Get-FileHash -Algorithm SHA256 $file).Hash.ToLower()
    # Two spaces, matching sha256sum output and what the release workflow writes.
    Set-Content -Path (Join-Path $dir 'SHA256SUMS') -Value "$hash  $asset" -NoNewline
}

Set-Content -Path (Join-Path $latest $asset) -Value 'genuine binary' -NoNewline
Write-Sums $latest

# The pinned release carries a distinguishable payload, so the test can tell
# which URL the installer actually fetched from.
Set-Content -Path (Join-Path $pinned $asset) -Value 'pinned binary' -NoNewline
Write-Sums $pinned

# TTID_BASE_URL redirects the fetch, so only the install dir and the PATH write
# need rewriting.
$script = Get-Content -Raw $src
$script = $script -replace '(?m)^\$dest\s*=.*$', ('$dest = ' + "'$dest'")
$script = $script -replace '(?m)^\s*\[Environment\]::SetEnvironmentVariable.*$', '    # PATH write suppressed for tests'
Set-Content -Path (Join-Path $work 'install.ps1') -Value $script

# Shadows the cmdlet: a function wins name resolution, so the installer's calls
# land here and read from the fake release instead of the network.
$shim = @'
$ErrorActionPreference = 'Stop'
function Invoke-WebRequest {
    param(
        [Parameter(Mandatory)][string]$Uri,
        [string]$OutFile,
        [switch]$UseBasicParsing
    )
    if (-not (Test-Path -LiteralPath $Uri)) { throw "404 Not Found: $Uri" }
    if ($OutFile) { Copy-Item -LiteralPath $Uri -Destination $OutFile -Force; return }
    [pscustomobject]@{ Content = (Get-Content -Raw -LiteralPath $Uri) }
}
. (Join-Path $PSScriptRoot 'install.ps1')
'@
Set-Content -Path (Join-Path $work 'harness.ps1') -Value $shim

$exe = Join-Path $dest 'ttid.exe'
$out = Join-Path $work 'out.txt'
$pass = 0
$fail = 0

function Invoke-Installer([hashtable]$Vars = @{}) {
    $env:TTID_BASE_URL = $releases
    foreach ($k in $Vars.Keys) { Set-Item -Path "env:$k" -Value $Vars[$k] }
    try {
        & pwsh -NoProfile -File (Join-Path $work 'harness.ps1') *> $out
        $code = $LASTEXITCODE
    } finally {
        Remove-Item -Path 'env:TTID_BASE_URL' -ErrorAction SilentlyContinue
        foreach ($k in $Vars.Keys) { Remove-Item -Path "env:$k" -ErrorAction SilentlyContinue }
    }
    return $code
}

function Check($name, $expected, $actual) {
    if ($expected -eq $actual) {
        Write-Host "  ok   $name"; $script:pass++
    } else {
        Write-Host "  FAIL $name (expected exit $expected, got $actual)"; $script:fail++
    }
}

function Assert($name, $condition) {
    if ($condition) { Write-Host "  ok   $name"; $script:pass++ }
    else { Write-Host "  FAIL $name"; $script:fail++ }
}

function Installed { Test-Path -LiteralPath $exe }
function Said($text) { (Get-Content -Raw $out) -match [regex]::Escape($text) }
function Payload { if (Installed) { (Get-Content -Raw -LiteralPath $exe).Trim() } else { '' } }

Write-Host 'install.ps1 fail-closed checks:'

Check 'matching checksum installs' 0 (Invoke-Installer)
Assert '  binary is installed' (Installed)
Assert '  verification is reported' (Said 'Checksum verified')
Remove-Item $exe -Force -ErrorAction SilentlyContinue

Set-Content -Path (Join-Path $latest $asset) -Value 'TAMPERED' -NoNewline
Check 'tampered binary aborts' 1 (Invoke-Installer)
Assert '  mismatch is reported' (Said 'Checksum mismatch')
Assert '  nothing is installed' (-not (Installed))
Set-Content -Path (Join-Path $latest $asset) -Value 'genuine binary' -NoNewline
Write-Sums $latest

Rename-Item (Join-Path $latest 'SHA256SUMS') 'SHA256SUMS.hidden'
Check 'unreachable SHA256SUMS aborts' 1 (Invoke-Installer)
Assert '  nothing is installed' (-not (Installed))
Rename-Item (Join-Path $latest 'SHA256SUMS.hidden') 'SHA256SUMS'

Set-Content -Path (Join-Path $latest 'SHA256SUMS') -Value 'deadbeef  some-other-asset' -NoNewline
Check 'asset absent from SHA256SUMS aborts' 1 (Invoke-Installer)
Assert '  nothing is installed' (-not (Installed))
Write-Sums $latest

Rename-Item (Join-Path $latest 'SHA256SUMS') 'SHA256SUMS.hidden'
Check 'TTID_SKIP_CHECKSUM=1 installs' 0 (Invoke-Installer @{ TTID_SKIP_CHECKSUM = '1' })
Assert '  the skip is warned about' (Said 'Skipping checksum verification')
Rename-Item (Join-Path $latest 'SHA256SUMS.hidden') 'SHA256SUMS'
Remove-Item $exe -Force -ErrorAction SilentlyContinue

# TTID_VERSION is the rollback path: it must fetch the pinned tag, not latest.
Check 'TTID_VERSION installs the pinned release' 0 (Invoke-Installer @{ TTID_VERSION = $pinnedTag })
Assert '  the pinned binary is the one installed' ((Payload) -eq 'pinned binary')
Remove-Item $exe -Force -ErrorAction SilentlyContinue

# A bare version is accepted; the leading 'v' is supplied.
Check 'TTID_VERSION without a leading v works' 0 (Invoke-Installer @{ TTID_VERSION = $pinnedTag.TrimStart('v') })
Assert '  the pinned binary is the one installed' ((Payload) -eq 'pinned binary')
Remove-Item $exe -Force -ErrorAction SilentlyContinue

Check 'unknown TTID_VERSION aborts' 1 (Invoke-Installer @{ TTID_VERSION = 'v0.0.00' })
Assert '  nothing is installed' (-not (Installed))

Write-Host '  ---'
Write-Host "  $pass passed, $fail failed"

Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
if ($fail -gt 0) { exit 1 }
