$ErrorActionPreference = 'Stop'
$script:UsbRoot = Split-Path $PSScriptRoot -Parent
if (-not (Test-Path -LiteralPath (Join-Path $script:UsbRoot 'config\settings.json'))) {
    throw 'USB project configuration is missing'
}
$folders = @{
    TEMP = 'tools\temp'
    TMP = 'tools\temp'
    CARGO_HOME = 'tools\cache\cargo'
    CARGO_TARGET_DIR = 'apps\manager\build-usb'
    npm_config_cache = 'tools\cache\npm'
    BUN_INSTALL_CACHE_DIR = 'tools\cache\bun'
}
foreach ($name in $folders.Keys) {
    $folder = Join-Path $script:UsbRoot $folders[$name]
    New-Item -ItemType Directory -Path $folder -Force | Out-Null
    [Environment]::SetEnvironmentVariable($name, $folder, 'Process')
}
$env:AI_USB_CONFIG = Join-Path $script:UsbRoot 'config\settings.json'
