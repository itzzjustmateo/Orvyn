$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'usb-environment.ps1')
& 'C:\Program Files\nodejs\node.exe' (Join-Path $script:UsbRoot 'tools\install-beui.mjs') card tabs animated-badge thinking-shimmer agent-progress input
if ($LASTEXITCODE -ne 0) { throw 'beui component installation failed' }
