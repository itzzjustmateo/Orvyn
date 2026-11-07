param([ValidateSet('before','after')][string]$Phase='before',[int]$Rounds=2)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'usb-environment.ps1')
$root=$script:UsbRoot
$fixture=Join-Path $root 'tools/temp/startup-benchmark'
foreach($dir in @('config','data','logs','backups')){New-Item -ItemType Directory (Join-Path $fixture $dir) -Force | Out-Null}
$release=if($Phase -eq 'before'){Join-Path $root 'dist'}else{Join-Path $root 'apps/manager/build-usb/release'}
$binary=if($Phase -eq 'before'){'AI-USB.exe'}else{'ai-usb-manager.exe'}
Copy-Item (Join-Path $release $binary) (Join-Path $fixture 'AI-USB.exe') -Force
Copy-Item (Join-Path $release 'WebView2Loader.dll') (Join-Path $fixture 'WebView2Loader.dll') -Force
$cfg=Get-Content (Join-Path $root 'config/settings.json') -Raw | ConvertFrom-Json
foreach($key in @('models','llamafile','web_ui')){$cfg.paths.$key=Join-Path $root $cfg.paths.$key}
$cfg.paths.data='data';$cfg.paths.logs='logs';$cfg.paths.backups='backups'
[IO.File]::WriteAllText((Join-Path $fixture 'config/settings.json'),($cfg|ConvertTo-Json -Depth 12))
Copy-Item (Join-Path $root 'config/model-catalog.json'),(Join-Path $root 'config/custom-models.json') (Join-Path $fixture 'config') -Force
if(Test-Path (Join-Path $root 'data/chats.json')){Copy-Item (Join-Path $root 'data/chats.json') (Join-Path $fixture 'data/chats.json') -Force}
$results=@()
for($round=1;$round -le $Rounds;$round++){
    $sessionPath=Join-Path $fixture 'data/desktop-session.json'
    if(Test-Path -LiteralPath $sessionPath){Remove-Item -LiteralPath $sessionPath}
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $process=Start-Process -WindowStyle Hidden -FilePath (Join-Path $fixture 'AI-USB.exe') -PassThru
    $window=$null;$loaded=$false
    try{
        while($watch.Elapsed.TotalSeconds -lt 180){
            $process.Refresh()
            if($process.HasExited){throw 'App exited before reporting ready'}
            if($null -eq $window -and $process.MainWindowHandle -ne 0){$window=$watch.Elapsed.TotalSeconds}
            if(Test-Path $sessionPath){try{$session=Get-Content $sessionPath -Raw | ConvertFrom-Json;if($session.ui_loaded){$loaded=$true;break}}catch{}}
            Start-Sleep -Milliseconds 150
        }
        if(-not $loaded){throw 'Start duurde langer dan 180 seconden'}
        $results += @{round=$round;window_seconds=$window;ready_signal_seconds=$watch.Elapsed.TotalSeconds;internal=$session.startup}
    }finally{
        if(-not $process.HasExited){
            $process.CloseMainWindow() | Out-Null
            if(-not $process.WaitForExit(10000)){
                # The test app can exit between the timeout and this call.
                try{$process.Kill();$process.WaitForExit(5000)|Out-Null}catch [System.InvalidOperationException]{}
            }
        }
    }
}
$results|ConvertTo-Json -Depth 8 | Tee-Object -FilePath (Join-Path $root ('tests/startup-'+$Phase+'.json'))
