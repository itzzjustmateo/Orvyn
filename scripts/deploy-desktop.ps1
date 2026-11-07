$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'usb-environment.ps1')
$root=$script:UsbRoot
$release=Join-Path $root 'apps\manager\build-usb\release'
foreach($parent in @($root,$release,(Join-Path $root 'dist'),(Join-Path $root 'backups'))){if(-not(Test-Path -LiteralPath $parent)){throw "Folder is missing: $parent"}}
$exe=Join-Path $release 'ai-usb-manager.exe'
$loader=Join-Path $release 'WebView2Loader.dll'
if(-not(Test-Path -LiteralPath $exe)){throw 'Bouw eerst de desktopapp'}
$backup=Join-Path $root ('backups\desktop-update-'+(Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $backup | Out-Null
foreach($relative in @('AI-USB.exe','AI-USB.next.exe','dist\AI-USB.exe','dist\WebView2Loader.dll','dist\SHA256SUMS.txt')){
    $path=Join-Path $root $relative
    if(Test-Path -LiteralPath $path){Copy-Item -LiteralPath $path -Destination (Join-Path $backup ($relative.Replace('\','-'))) -Force}
}
Copy-Item -LiteralPath $exe -Destination (Join-Path $root 'dist\AI-USB.exe') -Force
Copy-Item -LiteralPath $loader -Destination (Join-Path $root 'dist\WebView2Loader.dll') -Force
Copy-Item -LiteralPath $exe -Destination (Join-Path $root 'AI-USB.next.exe') -Force
try{Copy-Item -LiteralPath $exe -Destination (Join-Path $root 'AI-USB.exe') -Force;Remove-Item -LiteralPath (Join-Path $root 'AI-USB.next.exe')}catch{Write-Output 'Hoofdapp is geopend. Sluit de app en start via START.bat om de update te activeren.'}
$checks=foreach($name in @('AI-USB.exe','WebView2Loader.dll')){((Get-FileHash -LiteralPath (Join-Path $root ('dist\'+$name)) -Algorithm SHA256).Hash.ToLower()+'  '+$name)}
[IO.File]::WriteAllLines((Join-Path $root 'dist\SHA256SUMS.txt'),$checks,[Text.Encoding]::ASCII)
Write-Output "Distributie bijgewerkt. Vorige executables: $backup"
