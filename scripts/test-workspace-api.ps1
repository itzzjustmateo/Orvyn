param([switch]$Live)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'usb-environment.ps1')
$root=$script:UsbRoot
$parent=Join-Path $root 'tests'
if(-not(Test-Path -LiteralPath $parent)){throw 'Tests folder is missing'}
$fixture=Join-Path $parent ('workspace-api-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
foreach($name in @('config','data','models','logs','backups')){New-Item -ItemType Directory -Path (Join-Path $fixture $name) | Out-Null}
$release=Join-Path $root 'apps\manager\build-usb\release'
Copy-Item -LiteralPath (Join-Path $release 'ai-usb-manager.exe') -Destination (Join-Path $fixture 'AI-USB.exe')
Copy-Item -LiteralPath (Join-Path $release 'WebView2Loader.dll') -Destination (Join-Path $fixture 'WebView2Loader.dll')
$settings=@{paths=@{models='models';data='data';logs='logs';backups='backups';web_ui=(Join-Path $root 'apps\web-ui\dist');llamafile='unused.exe'};services=@{};model_policy=@{idle_seconds=1800}}
if($Live){
    $model=Join-Path $root 'models\desktop-active\llama32-1b.gguf'
    if(-not(Test-Path -LiteralPath $model)){throw 'Llama 3.2 1B is missing for the live test'}
    $listener=New-Object Net.Sockets.TcpListener([Net.IPAddress]::Loopback,0);$listener.Start();$port=$listener.LocalEndpoint.Port;$listener.Stop()
    $settings.services['stream-check']=@{name='Streamtest';port=$port;command=(Join-Path $root 'runtime\llamafile\llamafile-0.10.6.exe');args=@('--server','--model',$model,'--host','127.0.0.1','--port',[string]$port,'--ctx-size','4096','--n-predict','128','--gpu','disable','--nologo')}
}
[IO.File]::WriteAllText((Join-Path $fixture 'config\settings.json'),($settings|ConvertTo-Json -Depth 6))
$process=Start-Process -WindowStyle Hidden -FilePath (Join-Path $fixture 'AI-USB.exe') -PassThru
try{
    $sessionPath=Join-Path $fixture 'data\desktop-session.json'
    for($i=0;$i -lt 100 -and -not(Test-Path -LiteralPath $sessionPath);$i++){Start-Sleep -Milliseconds 200}
    $session=[IO.File]::ReadAllText($sessionPath)|ConvertFrom-Json
    $headers=@{'X-AIUSB-Session'=$session.token}
    function Api($path,$body=$null){
        if($null -eq $body){return Invoke-RestMethod -Uri ($session.url+$path) -Headers $headers -TimeoutSec 60}
        return Invoke-RestMethod -Uri ($session.url+$path) -Headers $headers -Method Post -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes(($body|ConvertTo-Json -Depth 10))) -TimeoutSec 60
    }
    $pc=Join-Path $fixture 'project'
    New-Item -ItemType Directory $pc | Out-Null
    [IO.File]::WriteAllText((Join-Path $pc 'hello.txt'),'before')
    $valid=Api '/api/opex/validate' @{folder=$pc;proposal=@{action='read';path='hello.txt'}}
    if(-not $valid.valid -or $valid.proposal.path -ne 'hello.txt'){throw 'Opex preflight failed'}
    foreach($proposal in @(@{action='run';command='echo BAD';approved=$true},@{action='read';path='../secret'},@{action='read';path='NUL.txt'},@{action='write';path='hello.txt'},@{action='run';command=17})){
        $blocked=$false
        try{$null=Api '/api/opex/validate' @{folder=$pc;proposal=$proposal}}catch{$blocked=$true}
        if(-not $blocked){throw 'Invalid Opex proposal accepted'}
    }
    if((Get-Content (Join-Path $pc 'hello.txt') -Raw) -ne 'before'){throw 'Preflight changed a file'}
    $read=Api '/api/opex/action' @{folder=$pc;action='read';path='hello.txt'}
    if($read.text -ne 'before'){throw 'Opex read failed'}
    $save=Api '/api/opex/action' @{folder=$pc;action='write';path='hello.txt';base='before';text='after';approved=$true}
    if([IO.File]::ReadAllText((Join-Path $pc 'hello.txt')) -ne 'after'){throw 'Opex write failed'}
    if([IO.File]::ReadAllText((Join-Path $save.backup 'original.txt')) -ne 'before'){throw 'Opex backup failed'}
    $blocked=$false
    try{$null=Api '/api/opex/action' @{folder=$pc;action='run';command='Write-Output SHOULD_NOT_RUN'}}catch{$blocked=$true}
    if(-not $blocked){throw 'Opex ran without approval'}
    $run=Api '/api/opex/action' @{folder=$pc;action='run';command='Write-Output OPEX_TERMINAL_OK';approved=$true}
    if($run.exit_code -ne 0 -or $run.output -notmatch 'OPEX_TERMINAL_OK'){throw 'Opex terminal failed'}
    New-Item -ItemType Directory (Join-Path $pc 'src') | Out-Null
    [IO.File]::WriteAllText((Join-Path $pc 'src/nested.txt'),'nested')
    $tree=Api '/api/opex/tree' @{folder=$pc}
    if('src/nested.txt' -notin $tree.files.path){throw 'Opex project tree omits nested file'}
    $blocked=$false
    try{$null=Api '/api/opex/run' @{folder=$pc;action='run';command='Write-Output SHOULD_NOT_RUN'}}catch{$blocked=$true}
    if(-not $blocked){throw 'Async terminal ran without approval'}
    $job=Api '/api/opex/run' @{folder=$pc;action='run';command='Write-Output OPEX_LIVE; Start-Sleep -Seconds 30; Write-Output SHOULD_NOT_FINISH';approved=$true}
    for($i=0;$i -lt 60;$i++){$status=Api '/api/opex/job' @{id=$job.id};if($status.output -match 'OPEX_LIVE'){break};Start-Sleep -Milliseconds 200}
    if($status.output -notmatch 'OPEX_LIVE' -or $status.status -ne 'running'){throw 'Live terminal output missing'}
    $null=Api '/api/opex/stop' @{id=$job.id}
    for($i=0;$i -lt 60;$i++){$status=Api '/api/opex/job' @{id=$job.id};if($status.status -ne 'running'){break};Start-Sleep -Milliseconds 200}
    if(-not $status.cancelled -or $status.output -match 'SHOULD_NOT_FINISH'){throw 'Opex stop did not terminate command'}
    $job=Api '/api/opex/run' @{folder=$pc;action='run';command='Write-Output OPEX_JOB_DONE; exit 7';approved=$true}
    for($i=0;$i -lt 60;$i++){$status=Api '/api/opex/job' @{id=$job.id};if($status.status -ne 'running'){break};Start-Sleep -Milliseconds 200}
    if($status.exit_code -ne 7 -or $status.output -notmatch 'OPEX_JOB_DONE'){throw 'Async terminal result missing'}
    Add-Type -AssemblyName System.Drawing
    $icon=[Drawing.Icon]::ExtractAssociatedIcon((Join-Path $fixture 'AI-USB.exe'))
    if($null -eq $icon){throw 'Executable icon missing'}
    $icon.ToBitmap().Save((Join-Path $root 'tests/orvyn-executable-icon.png'))
    $icon.Dispose()
    $catalog=Api '/api/catalog'
    if($catalog.runtime.idle_seconds -ne 1800){throw 'Timeout is not 30 minutes'}
    $index=Api '/api/index/scan' @{}
    if($null -eq $index.folders -or $null -eq $index.files){throw 'Folder index did not return a valid empty scan'}
    $provider=Api '/api/providers' @{kind='openai';name='Test API';model='test-model';category='chat';api_key='native-test-secret'}
    $providers=Api '/api/providers'
    if(($providers|ConvertTo-Json -Depth 8) -match 'native-test-secret|protected_key'){throw 'API-sleutel zichtbaar in providerlijst'}
    if([IO.File]::ReadAllText((Join-Path $fixture 'config/api-providers.json')) -match 'native-test-secret'){throw 'API-sleutel onversleuteld opgeslagen'}

    $catalog=Api '/api/catalog'
    if(-not $catalog.models.($provider.id).remote){throw 'API model is missing from the catalog'}
    $null=Api '/api/providers/remove' @{id=$provider.id}
    if((Api '/api/providers').Count -gt 0){throw 'API provider was not removed'}
    $hardware=Api '/api/hardware'
    if($hardware.ram_bytes -le 0){throw 'RAM was not reported'}
    $doc=Api '/api/documents' @{name='test.txt';text='De Tweede Wereldoorlog begon in 1939 en eindigde in 1945.'}
    $hits=Api '/api/documents/search' @{query='Wereldoorlog';id=$doc.id}
    if($hits[0].name -ne 'test.txt' -or $hits[0].section -ne 1){throw 'Source reference is missing'}
    $usage=Api '/api/disk-usage'
    if($usage.documents -le 0){throw 'Document storage was not measured'}
    $state=@{chats=@();preferences=@{style='kort';language='Nederlands';favorites=@('qwen3')};revision=(Api '/api/storage').revision}
    $null=Invoke-RestMethod -Uri ($session.url+'/api/storage') -Headers $headers -Method Put -ContentType 'application/json' -Body ($state|ConvertTo-Json -Depth 6)
    $saved=Api '/api/storage'
    if($saved.preferences.style -ne 'kort'){throw 'Voorkeuren niet bewaard'}
    $export=Api '/api/export' @{extension='md';text='# Exporttest'}
    if(-not(Test-Path -LiteralPath $export.path)){throw 'Export was not saved to USB'}
    $code=Api '/api/opex/export' @{name='example.py';text='print("Orvyn")'}
    if(-not(Test-Path -LiteralPath $code.path) -or [IO.Path]::GetExtension($code.path) -ne '.py'){throw 'Opex export failed'}
    $backup=Api '/api/backup' @{}
    if(-not(Test-Path -LiteralPath $backup.path)){throw 'Backup from settings failed'}
    $logo=Invoke-WebRequest -UseBasicParsing -Uri ($session.url+'/orvyn-logo.png')
    if($logo.Headers['Content-Type'] -notlike 'image/png*'){throw 'Orvyn-logo ontbreekt'}
    $read=Api '/api/documents/get' @{id=$doc.id}
    if($read.name -ne 'test.txt'){throw 'Failed to retrieve document'}
    $stream=Invoke-WebRequest -UseBasicParsing -Uri ($session.url+'/api/chat-stream') -Headers $headers -Method Post -ContentType 'application/json' -Body '{"model":"missing","messages":[]}' -TimeoutSec 20
    if($stream.Headers['Content-Type'] -notlike 'text/event-stream*' -or $stream.Content -notmatch '\[DONE\]'){throw 'SSE route did not close correctly'}
    if($Live){
        $answer=Invoke-WebRequest -UseBasicParsing -Uri ($session.url+'/api/chat-stream') -Headers $headers -Method Post -ContentType 'application/json' -Body '{"model":"stream-check","messages":[{"role":"user","content":"Groet me kort in het Nederlands."}],"max_tokens":32}' -TimeoutSec 240
        if($answer.Content -match '"error"\s*:' -or $answer.Content -notmatch '"content"\s*:\s*"[^" ]' -or $answer.Content -notmatch '\[DONE\]'){throw ('Live modelstream mislukt: '+$answer.Content)}
        Write-Output 'Live Llama-model gaf echte SSE-tekstfragmenten terug.'
    }
    for($i=0;$i -lt 50;$i++){$ready=[IO.File]::ReadAllText($sessionPath)|ConvertFrom-Json;if($ready.ui_loaded){break};Start-Sleep -Milliseconds 200}
    if(-not $ready.ui_loaded){throw 'WebView-interface niet geladen'}
    $diagnostics=Api '/api/diagnostics/startup'
    if(-not $diagnostics.complete -or $diagnostics.phases.Count -lt 5 -or $diagnostics.total_ms -le 0){throw 'Startup diagnostics are missing'}
    if(($diagnostics|ConvertTo-Json -Depth 12).Contains($session.token)){throw 'Diagnose bevat sessiesleutel'}
    $training=Api '/api/training'
    if($training.candidates.Count -ne 0 -or $training.dataset.issues.Count -eq 0){throw 'Lege trainingsstatus onjuist'}
    $trainingData=Join-Path $fixture 'data/training/orvyn-1/v2'
    New-Item -ItemType Directory $trainingData -Force | Out-Null
    Copy-Item (Join-Path $root 'data/training/orvyn-1/v2/train.jsonl'),(Join-Path $root 'data/training/orvyn-1/v2/eval.jsonl') $trainingData
    $training=Api '/api/training'
    if($training.dataset.counts.train.pc -ne 28 -or $training.dataset.counts.eval.code -ne 12 -or $training.dataset.issues.Count -ne 0){throw 'Trainingsdataset onjuist'}
    $examples=Api '/api/training/examples' @{split='eval';domain='pc'}
    if($examples.examples.Count -ne 12){throw 'Trainingsvoorbeelden ontbreken'}
    $candidatePath=Join-Path $fixture 'models/training/orvyn1-candidate-fixture'
    New-Item -ItemType Directory $candidatePath -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $candidatePath 'evaluation.json'),'{"steps":24,"baseline_loss":{"nl":2},"candidate_loss":{"nl":1.5}}')
    [IO.File]::WriteAllText((Join-Path $candidatePath 'answers.json'),'{"baseline":[{"id":"same","answer":"before"}],"candidate":[{"id":"same","answer":"after"}]}')
    $candidate=Api '/api/training/candidate' @{id='orvyn1-candidate-fixture'}
    if($candidate.answers.candidate[0].answer -ne 'after' -or $candidate.evaluation.steps -ne 24){throw 'Candidate comparison is missing'}
    $blocked=$false
    try{$null=Api '/api/training/candidate' @{id='../config'}}catch{$blocked=$true}
    if(-not $blocked){throw 'Kandidaatroute laat padtraversal toe'}
    Write-Output 'Native API/WebView test passed: 1800s, hardware, documents, sources, storage, preferences, export, and SSE error handling.'
}finally{
    if(-not $process.HasExited){$null=$process.CloseMainWindow();if(-not $process.WaitForExit(10000)){$process.Kill();$process.WaitForExit()}}
    Write-Output "Geïsoleerde testgegevens: $fixture"
}
