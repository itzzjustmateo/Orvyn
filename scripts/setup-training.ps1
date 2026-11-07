$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'usb-environment.ps1')
$root=$script:UsbRoot
$env:PIP_CACHE_DIR=Join-Path $root 'tools/cache/pip'
$env:PYTHONDONTWRITEBYTECODE='1'
$env:PIP_NO_COMPILE='1'
$venv=Join-Path $root 'tools/training-venv'
& python.exe -m venv --system-site-packages --without-pip $venv
if($LASTEXITCODE -ne 0){throw 'Failed to create USB training environment'}
$python=Join-Path $venv 'Scripts/python.exe'
& python.exe -m pip --python $python install --no-compile 'peft>=0.17,<1' 'gguf>=0.17,<1'
if($LASTEXITCODE -ne 0){throw 'Failed to install training dependencies'}
& python.exe -m pip --python $python freeze | Set-Content (Join-Path $root 'tools/training/environment.txt')
& $python (Join-Path $root 'tools/training/train_orvyn.py')
if($LASTEXITCODE -ne 0){throw 'Training validation failed'}
