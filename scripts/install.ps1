# TTID installer for Windows.
#   irm https://github.com/d31ma/TTID/releases/latest/download/install.ps1 | iex
# Downloads the latest Windows binary from GitHub releases, verifies its
# checksum, and installs it under %LOCALAPPDATA%\TTID (added to your user PATH).
$ErrorActionPreference = 'Stop'

$repo = 'd31ma/TTID'
$releases = if ($env:TTID_BASE_URL) { $env:TTID_BASE_URL } else { "https://github.com/$repo/releases" }

# Defaults to the latest release. Set $env:TTID_VERSION to install a specific one
# -- this is the rollback path when a release turns out bad:
#   $env:TTID_VERSION = '26.28.02'; irm .../install.ps1 | iex
# Tags carry a leading 'v', which is added here when it is left off.
if ($env:TTID_VERSION) {
    $tag = $env:TTID_VERSION
    if ($tag -notmatch '^v') { $tag = "v$tag" }
    $base = "$releases/download/$tag"
} else {
    $base = "$releases/latest/download"
}

$asset = 'ttid-windows-x64.exe'

$dest = Join-Path $env:LOCALAPPDATA 'TTID'
New-Item -ItemType Directory -Force -Path $dest | Out-Null
$exe = Join-Path $dest 'ttid.exe'

Write-Host "Downloading $asset..."
try {
    Invoke-WebRequest -Uri "$base/$asset" -OutFile $exe
} catch {
    if ($env:TTID_VERSION) {
        throw "Failed to download $asset. Is TTID_VERSION=$($env:TTID_VERSION) a released version? See $releases. $_"
    }
    throw "Failed to download $asset from $base/$asset. $_"
}

# Verify the download against the release SHA256SUMS. This fails closed: any
# step that cannot complete aborts the install rather than skipping the check.
# Set $env:TTID_SKIP_CHECKSUM = '1' to install without verification.
if ($env:TTID_SKIP_CHECKSUM -eq '1') {
    Write-Warning 'Skipping checksum verification (TTID_SKIP_CHECKSUM=1).'
} else {
    # Only the download is wrapped, so a genuine mismatch below is never caught
    # and downgraded to a warning.
    try {
        $sums = (Invoke-WebRequest -Uri "$base/SHA256SUMS" -UseBasicParsing).Content
    } catch {
        Remove-Item $exe -Force -ErrorAction SilentlyContinue
        throw "Cannot verify ${asset}: failed to download SHA256SUMS. $_"
    }

    $line = ($sums -split "`n") |
        Where-Object { $_ -match "\s\*?$([regex]::Escape($asset))\s*$" } |
        Select-Object -First 1
    if (-not $line) {
        Remove-Item $exe -Force -ErrorAction SilentlyContinue
        throw "Cannot verify ${asset}: no entry for it in SHA256SUMS."
    }

    $expected = ($line -split '\s+')[0].ToLower()
    $actual = (Get-FileHash -Algorithm SHA256 $exe).Hash.ToLower()
    if ($expected -ne $actual) {
        Remove-Item $exe -Force -ErrorAction SilentlyContinue
        throw "Checksum mismatch for ${asset} (expected $expected, got $actual). Aborting."
    }
    Write-Host 'Checksum verified.'
}

# Add install dir to the user PATH if it isn't already there.
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath -notlike "*$dest*") {
    [Environment]::SetEnvironmentVariable('Path', "$userPath;$dest", 'User')
    Write-Host "Added $dest to your user PATH (restart your terminal to pick it up)."
}

Write-Host "Installed ttid to $exe"
Write-Host "Run 'ttid --help' to get started."
