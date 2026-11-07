$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'usb-environment.ps1')
$script:Cargo = (Get-Command cargo.exe -ErrorAction Stop).Source
$rustInfo = & rustc.exe -vV
if ($LASTEXITCODE -ne 0) { throw 'Rust compiler could not be queried.' }
$hostLine = $rustInfo | Where-Object { $_ -like 'host: *' }
$script:RustHost = $hostLine.Substring(6).Trim()
if ($script:RustHost -like '*-windows-msvc') {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere)) { throw 'Visual Studio C++ Build Tools are required for MSVC Rust.' }
    $installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $installation) { throw 'Install the Visual Studio Desktop development with C++ workload and Windows SDK.' }
    $vcvars = Join-Path $installation 'VC/Auxiliary/Build/vcvars64.bat'
    # Import the compiler environment into this process only; no global PATH changes.
    $compilerEnvironment = & $env:ComSpec /d /c "call `"$vcvars`" >nul && set"
    if ($LASTEXITCODE -ne 0) { throw 'Visual Studio compiler environment setup failed.' }
    foreach ($line in $compilerEnvironment) {
        if ($line -match '^([^=]+)=(.*)$') { [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], 'Process') }
    }
    foreach ($tool in @('link.exe','rc.exe')) { $null = Get-Command $tool -ErrorAction Stop }
} elseif ($script:RustHost -like '*-windows-gnu') {
    $tools = Join-Path $script:UsbRoot 'tools/llvm-mingw-20260922-msvcrt-x86_64/bin'
    foreach ($tool in @('llvm-dlltool.exe','llvm-windres.exe')) {
        if (-not (Test-Path -LiteralPath (Join-Path $tools $tool))) { throw "GNU Rust tool is missing: $tool in $tools" }
    }
    $env:ORVYN_WINDRES = Join-Path $tools 'llvm-windres.exe'
    $env:RUSTFLAGS = '-C dlltool=' + (Join-Path $tools 'llvm-dlltool.exe') + ' -C link-arg=-B' + $tools + '\'
} else { throw "Unsupported Windows Rust host: $script:RustHost" }
