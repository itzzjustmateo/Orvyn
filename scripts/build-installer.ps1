param([string]$Compiler,[switch]$WithoutOrvyn1)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'usb-environment.ps1')
$root=$script:UsbRoot
if(-not $Compiler){
    $candidates=@(
        (Join-Path $root 'tools/innosetup/ISCC.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs/Inno Setup 6/ISCC.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs/Inno Setup 7/ISCC.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6/ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 7/ISCC.exe')
    )
    $command=Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if($command){$candidates=@($command.Source)+$candidates}
    $Compiler=$candidates | Where-Object {Test-Path -LiteralPath $_} | Select-Object -First 1
}
if(-not $Compiler -or -not(Test-Path -LiteralPath $Compiler)){throw 'Inno Setup compiler not found. Pass -Compiler with the path to an existing ISCC.exe.'}
& (Join-Path $PSScriptRoot 'package-portable.ps1') -WithoutOrvyn1:$WithoutOrvyn1
$version=(Get-Content -LiteralPath (Join-Path $root 'config/settings.example.json') -Raw | ConvertFrom-Json).app.version
if($version -notmatch '^\d+\.\d+\.\d+$'){throw 'Expected a numeric major.minor.patch application version.'}
$suffix=if($WithoutOrvyn1){'-No-Orvyn-1'}else{''}
$output=Join-Path $root ('dist/Orvyn-Setup-'+$version+$suffix+'.exe')
& $Compiler /Q ('/DAppVersion='+$version) ('/DOutputSuffix='+$suffix) ('/DPackage='+ (Join-Path $root ('dist/Orvyn-portable'+$suffix))) (Join-Path $root 'apps/manager/packaging/installer.iss')
if($LASTEXITCODE -ne 0){throw 'Installer compilation failed.'}
if(-not(Test-Path -LiteralPath $output)){throw 'Installer output is missing.'}
# Replace the old generic installer as well, so it cannot install obsolete models.
if(-not $WithoutOrvyn1){Copy-Item -LiteralPath $output -Destination (Join-Path $root 'dist/installer.exe') -Force}
foreach($name in @('INSTALL.txt','LEESMIJ.txt')){Copy-Item -LiteralPath (Join-Path $root 'apps/manager/packaging/INSTALL.txt') -Destination (Join-Path $root ('dist/'+$name)) -Force}
$hash=(Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLower()
[IO.File]::WriteAllText(($output+'.sha256'),($hash+'  '+[IO.Path]::GetFileName($output)+"`n"),[Text.Encoding]::ASCII)
Write-Output ('Installer ready: '+$output)
Write-Output ('Size: '+[Math]::Round((Get-Item -LiteralPath $output).Length/1MB,1)+' MiB')
