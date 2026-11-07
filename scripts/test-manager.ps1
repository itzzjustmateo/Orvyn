$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'rust-environment.ps1')
& $script:Cargo test --manifest-path (Join-Path $script:UsbRoot 'apps/manager/Cargo.toml') --release --locked --offline --jobs 4 --target-dir (Join-Path $script:UsbRoot 'apps/manager/build-usb') -- --test-threads=2
if($LASTEXITCODE -ne 0){throw 'Rust tests failed'}
