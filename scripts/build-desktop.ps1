$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'rust-environment.ps1')
$root = Split-Path $PSScriptRoot -Parent
# Run from apps\manager; paths are derived from the USB, not a fixed drive letter.
& $script:Cargo build --manifest-path (Join-Path $root 'apps\manager\Cargo.toml') --release --locked --offline --jobs 4 --target-dir (Join-Path $root 'apps\manager\build-usb')
if ($LASTEXITCODE -ne 0) { throw 'Rust build failed' }
$release=Join-Path $root 'apps\manager\build-usb\release'
$loaders=@(Get-ChildItem -LiteralPath (Join-Path $release 'build') -Recurse -Filter WebView2Loader.dll | Where-Object { $_.Directory.Name -eq 'x64' })
if(-not $loaders.Count){throw 'WebView2 loader is missing'}
Copy-Item -LiteralPath $loaders[0].FullName -Destination (Join-Path $release 'WebView2Loader.dll') -Force
