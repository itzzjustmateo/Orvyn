# Orvyn 0.7.0 — AI Model Library

November 5, 2026

## Latest development changes

Based on commit `84e5752` (`feat: model libary`), the latest project commit.

- **Discover models from Hugging Face.** Search for GGUF models directly from the AI library.
- **Browse by category.** Find models for Chat, Code, Reasoning, and Vision / OCR.
- **See more before downloading.** Community model cards show their source, file size, and download count when available.
- **Download to your portable installation.** Models use the existing download queue with SHA-256 verification.
- **Support for vision model files.** The library can include an accompanying projector file when one is available.
- **Keep your library configuration in backups.** Backup and restore now include the store model catalog.

Community models are shown as unrated unless a quality assessment is available. Searching and downloading require an internet connection; model compatibility and performance depend on the selected model and your hardware.

## Installer options

- **Standard:** includes Orvyn 1 for local chat.
- **Without Orvyn 1:** includes no model weights or Orvyn 1 profile. Connect an API or download a model to get started.

## Build status

The model-library changes above are confirmed in source. The generated 0.7.0 installers reuse the existing desktop executable rather than a fresh native build of the latest commit, so these changes have not been verified in those installers. Both installer variants passed fresh-install and UI-startup checks with their expected model counts.

## English text and licensing update

Application labels, accessibility text, prompts, errors, scripts, and documentation have been translated into English. English Wikipedia search and English number formatting are used. Response languages remain configurable, and existing preference values are preserved. The application code and documentation were initially offered under MIT; bundled model and runtime licenses remain separate. Intentional multilingual training examples are preserved.

Validation: English web bundle built; TypeScript and browser integration passed. Native Rust tests could not run because the offline cache lacks base64. Existing installer EXEs have not been rebuilt with this update.


## Redistribution license update — November 6, 2026

New covered Orvyn material uses the Orvyn Free Redistribution License 1.0: free sharing is permitted, while paid redistribution requires separate written permission. Earlier MIT permissions and third-party licenses remain in effect for their covered material. Package metadata, bundled notices, and the installer license screen now reference the application license. Existing installer EXEs must be rebuilt to carry these changes. See [licensing](LICENSING.md).
