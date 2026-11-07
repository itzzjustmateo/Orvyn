$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'usb-environment.ps1')
$node = (Get-Command node.exe -ErrorAction Stop).Source
$ui = Join-Path $script:UsbRoot 'apps\web-ui'
$process = Start-Process -FilePath $node -ArgumentList 'build-desktop.mjs' -WorkingDirectory $ui -NoNewWindow -PassThru
$handle = $process.Handle
$process.WaitForExit()
if ($process.ExitCode -ne 0) { throw 'UI build failed' }
