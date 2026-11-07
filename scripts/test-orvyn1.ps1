$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'usb-environment.ps1')
$root=$script:UsbRoot
$fixture=Join-Path $root ('tests/orvyn1-'+[guid]::NewGuid().ToString('N'))
foreach($dir in @('config','data','logs','backups')){New-Item -ItemType Directory (Join-Path $fixture $dir) -Force | Out-Null}
Copy-Item (Join-Path $root 'apps/manager/build-usb/release/ai-usb-manager.exe') (Join-Path $fixture 'AI-USB.exe')
Copy-Item (Join-Path $root 'apps/manager/build-usb/release/WebView2Loader.dll') $fixture
$catalog=Get-Content (Join-Path $root 'config/custom-models.json') -Raw | ConvertFrom-Json
$entry=$catalog.models | Where-Object id -eq 'orvyn-1'
[IO.File]::WriteAllText((Join-Path $fixture 'config/custom-models.json'),(@{version=1;models=@($entry)}|ConvertTo-Json -Depth 12))
$listener=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0)
$listener.Start();$port=$listener.LocalEndpoint.Port;$listener.Stop()
$settings=@{paths=@{models=(Join-Path $root 'models/desktop-active');data='data';logs='logs';backups='backups';web_ui=(Join-Path $root 'apps/web-ui/dist');llamafile=(Join-Path $root 'runtime/llamafile/llamafile-0.10.6.exe')};model_policy=@{idle_seconds=1800};services=@{'orvyn-1'=@{name='Orvyn 1';port=$port;command=(Join-Path $root 'runtime/llamafile/llamafile-0.10.6.exe');args=@('--server','--model',(Join-Path $root 'models/desktop-active/llama32-1b.gguf'),'--host','127.0.0.1','--port',[string]$port,'--ctx-size','4096','--n-predict','768','--gpu','disable','--parallel','1','--nologo')}}}
[IO.File]::WriteAllText((Join-Path $fixture 'config/settings.json'),($settings|ConvertTo-Json -Depth 10))
$process=Start-Process -WindowStyle Hidden -FilePath (Join-Path $fixture 'AI-USB.exe') -PassThru
try {
    $sessionPath=Join-Path $fixture 'data/desktop-session.json'
    for($i=0;$i -lt 150 -and -not(Test-Path $sessionPath);$i++){Start-Sleep -Milliseconds 200}
    $session=Get-Content $sessionPath -Raw | ConvertFrom-Json
    $headers=@{'X-AIUSB-Session'=$session.token}
    $catalog=Invoke-RestMethod ($session.url+'/api/catalog') -Headers $headers
    if(-not $catalog.models.'orvyn-1'.installed){throw 'Orvyn 1 is unavailable'}
    $website=Invoke-RestMethod ($session.url+'/api/web-read') -Headers $headers -Method Post -ContentType 'application/json' -Body '{"url":"https://example.com"}' -TimeoutSec 60
    if($website.text -notmatch 'Example Domain'){throw 'Web page was not read'}
    $cacheWatch=[Diagnostics.Stopwatch]::StartNew()
    $again=Invoke-RestMethod ($session.url+'/api/web-read') -Headers $headers -Method Post -ContentType 'application/json' -Body '{"url":"https://example.com"}' -TimeoutSec 60
    $cacheWatch.Stop()
    if($again.text -ne $website.text){throw 'Webcache verschilt'}
    $blocked=$false
    try{Invoke-RestMethod ($session.url+'/api/web-read') -Headers $headers -Method Post -ContentType 'application/json' -Body '{"url":"https://127.0.0.1"}' -TimeoutSec 30 | Out-Null}catch{$blocked=$true}
    if(-not $blocked){throw 'Local address was not blocked'}
    @{url=$website.url;text=$website.text;cached_seconds=$cacheWatch.Elapsed.TotalSeconds}|ConvertTo-Json | Set-Content (Join-Path $root 'tests/orvyn-web-result.json')
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $body=@{model='orvyn-1';messages=@(@{role='user';content='What is your name? Answer in a short English sentence.'});max_tokens=48}|ConvertTo-Json -Depth 5
    $reply=Invoke-RestMethod ($session.url+'/api/chat') -Headers $headers -Method Post -ContentType 'application/json' -Body $body -TimeoutSec 300
    $watch.Stop()
    $text=$reply.choices[0].message.content
    if([string]::IsNullOrWhiteSpace($text)){throw 'No response'}
    if($text -notmatch 'Orvyn'){throw "Model profile is not recognizable: $text"}
    $warm=[Diagnostics.Stopwatch]::StartNew()
    $math=@{model='orvyn-1';messages=@(@{role='user';content='What is 2 + 3? Return only the number.'});max_tokens=16}|ConvertTo-Json -Depth 5
    $answer=Invoke-RestMethod ($session.url+'/api/chat') -Headers $headers -Method Post -ContentType 'application/json' -Body $math -TimeoutSec 120
    $warm.Stop()
    if($answer.choices[0].message.content -notmatch '5'){throw 'Simple arithmetic test failed'}
    @{model='Orvyn 1';warm_request_seconds=$warm.Elapsed.TotalSeconds;warm_response=$answer.choices[0].message.content;response=$text;cold_request_seconds=$watch.Elapsed.TotalSeconds;usage=$reply.usage}|ConvertTo-Json -Depth 6 | Tee-Object -FilePath (Join-Path $root 'tests/orvyn1-live-result.json')
    Invoke-RestMethod ($session.url+'/api/models/unload') -Headers $headers -Method Post -ContentType 'application/json' -Body '{}' | Out-Null
}finally{
    if(-not $process.HasExited){Stop-Process -Id $process.Id -Force}
}
