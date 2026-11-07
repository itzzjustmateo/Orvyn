# Cargo tar timestamps can fail on exFAT. Verify archives and unpack without timestamps.
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'usb-environment.ps1')
# Cargo must populate the registry index as well as crate archives for offline resolution.
& cargo.exe fetch --manifest-path (Join-Path $script:UsbRoot 'apps/manager/Cargo.toml') --locked
if ($LASTEXITCODE -ne 0) { throw 'Locked Cargo dependency fetch failed.' }
$registry = 'index.crates.io-1949cf8c6b5b557f'
$cache = Join-Path $env:CARGO_HOME ('registry\cache\' + $registry)
$sources = Join-Path $env:CARGO_HOME ('registry\src\' + $registry)
New-Item -ItemType Directory -Path $cache,$sources -Force | Out-Null
$lock = Get-Content -LiteralPath (Join-Path $script:UsbRoot 'apps\manager\Cargo.lock') -Raw
$count = 0
foreach ($block in ($lock -split '\[\[package\]\]')) {
    if ($block -notmatch '(?m)^checksum = "([a-f0-9]+)"') { continue }
    $checksum=$Matches[1]
    if ($block -notmatch '(?m)^name = "([^"]+)"') { continue }; $name=$Matches[1]
    if ($block -notmatch '(?m)^version = "([^"]+)"') { continue }; $version=$Matches[1]
    $filename="$name-$version.crate"
    $archive=Join-Path $cache $filename
    $target=Join-Path $sources "$name-$version"
    if (Test-Path -LiteralPath (Join-Path $target '.cargo-ok')) { continue }
    if (-not (Test-Path -LiteralPath $archive)) {
        $existing=Join-Path $env:USERPROFILE ('.cargo\registry\cache\'+$registry+'\'+$filename)
        if (Test-Path -LiteralPath $existing) { Copy-Item -LiteralPath $existing -Destination $archive }
        else {
            & curl.exe --silent --show-error --fail --location --output $archive "https://static.crates.io/crates/$name/$filename"
            if ($LASTEXITCODE -ne 0) { throw "Download failed: $name" }
        }
    }
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLower() -ne $checksum) { throw "Checksum failed: $name" }
    & tar.exe -xmzf $archive -C $sources
    if ($LASTEXITCODE -ne 0) { throw "Extraction failed: $name" }
    [IO.File]::WriteAllText((Join-Path $target '.cargo-ok'),'{"v":1}')
    $count++
}
"Project Cargo cache prepared: $count packages"
