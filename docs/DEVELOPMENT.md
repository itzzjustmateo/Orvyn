# Development and packaging

## Prerequisites

For the supported MSVC build, use 64-bit Windows, Node.js 22, Rust 1.97.1 with the `x86_64-pc-windows-msvc` toolchain, and Visual Studio's Desktop development with C++ workload including a Windows SDK. Existing system tools can be used; project dependencies, caches, and temporary files remain under the repository. WebView2 is required to run the app.

The desktop scripts detect the installed Rust host. MSVC builds use Visual Studio tools discovered by `vswhere`; GNU builds use the project-local LLVM/MinGW tools at the path in `scripts/rust-environment.ps1`. That script changes compiler paths for the current process only.

## Build and run

From the repository root:

```powershell
.\scripts\initialize-config.ps1
. .\scripts\usb-environment.ps1
Push-Location apps/web-ui
npm ci
Pop-Location
.\scripts\build-ui.ps1
.\scripts\prepare-cargo-cache.ps1
.\scripts\build-desktop.ps1
.\scripts\deploy-desktop.ps1
.\dist\AI-USB.exe
```

Configuration initialization preserves existing settings. Cache preparation fetches the registry index and dependencies from `Cargo.lock`. Native builds and tests use `--locked --offline` afterward. Close the app before deploying an updated executable. The root-level development launchers have been removed; run `dist/AI-USB.exe` directly.

The source repository contains neither model weights nor the inference runtime. An API connection works without a local model. For local inference, provide the compatible runtime and model files described below.

## Local checks

After building the web bundle:

```powershell
. .\scripts\usb-environment.ps1
node apps/web-ui/node_modules/typescript/bin/tsc --project apps/web-ui/tsconfig.json --noEmit --incremental false
node tests/chat-client.mjs
node tests/workspace-ui.mjs
node tests/opex-work-ui.mjs
.\scripts\test-manager.ps1
```

Browser tests use Microsoft Edge, mocked model responses, and generated fixtures. Native tests exercise local file operations, backups, streaming, providers, and PowerShell. Tests requiring a live Qwen service or live Hugging Face search are explicitly ignored by default. GitHub CI runs these source checks and the native release build; see [GitHub setup](GITHUB.md).

`scripts/test-workspace-api.ps1` and `scripts/test-installer.ps1` are additional integration checks requiring their corresponding runtime/build inputs. The installer test currently assumes the standard model-bundled package.

## Portable packages and installers

For bundled Orvyn 1 packages, provide:

- `runtime/llamafile/llamafile-0.10.6.exe`.
- `models/desktop-active/llama32-1b.gguf` and its `.verified.json` metadata.
- The original model and runtime notices, already listed under `apps/manager/packaging/licenses`.

The model source URL and expected SHA-256 are in `config/custom-models.example.json`. Preserve those checks and the upstream licenses. The package without Orvyn 1 still requires the runtime, but does not require or copy model weights.

After building and deploying the current executable:

```powershell
# Standard package and installer, with Orvyn 1
.\scripts\package-portable.ps1
.\scripts\build-installer.ps1

# Separate staging folder and installer, without Orvyn 1
.\scripts\package-portable.ps1 -WithoutOrvyn1
.\scripts\build-installer.ps1 -WithoutOrvyn1
```

The installer requires an existing Inno Setup compiler. Pass `-Compiler` to `build-installer.ps1` if it is not detected automatically. Source and notices are under `apps/manager/packaging`; generated installers and SHA-256 files remain under `dist` and are excluded from Git.

Always rebuild both the web bundle and desktop executable before packaging a new source revision. Older 0.7.0 packages reused an earlier executable; the repaired native build does not automatically update existing installer EXEs. Record the source commit and validation results with each release.

## Source layout

| Path | Purpose |
| --- | --- |
| `apps/web-ui` | React, TypeScript, and CSS interface |
| `apps/manager` | Rust desktop host and local APIs |
| `scripts` | Environment, build, test, and packaging commands |
| `config/*.example.json` | Clean configuration templates with the Orvyn 1 profile |
| `tests` | Source tests; generated fixtures and screenshots are ignored |
| `tools/training` | Optional CPU training utilities |
| `.github` | CI workflow, collaboration templates, and README assets |

Conversations, models, executables, runtimes, caches, WebView profiles, credentials, personal configuration, logs, and backups are not source files and must remain excluded from Git. Review the source-only file list in GitHub Desktop before publishing or pushing. Do not force-add ignored private files.

## Language and training

The interface, prompts, messages, and documentation use English. Response language remains configurable, and legacy internal preference values are retained for compatibility. Multilingual training and evaluation examples keep their original language to preserve their purpose. See the [training notes](../tools/training/README.txt) before running an experiment.
