# AI USB workspace

After updating, close the app and open `START.bat`. Models unload after **30 minutes of inactivity**. Switching models, canceling, or closing the app may stop them sooner.

## Models

- **Smart model selection** uses local keyword rules to choose an installed model for code, reasoning, chat, or images. It considers memory recommendations and shows the selected model with the response. Manual selection remains available.
- **Hardware advice** displays the CPU, logical threads, and available RAM. Estimated model memory includes context overhead and is not a guarantee.
- **Compare with** sends the same question to two installed models sequentially and displays responses side by side. Canceling stops the request.
- **Stars** are editorial estimates, not measured benchmarks. Dutch-language ability, code, reasoning, and speed have separate estimates. Speed uses download size as a rough indicator and depends on the computer.
- **Favorites** appear first and have their own library tab.
- The **Hugging Face library** discovers GGUF models for Chat, Code, Reasoning, and Vision / OCR. Available models depend on upstream search results. Download sizes and hardware requirements vary. Vision projectors are downloaded with their model and verified separately.
- Downloads support a **queue**. Pausing or encountering an error stops the queue; resume from the library. The queue lasts for the current app session. Partial downloads remain on the USB after closing.

## Documents, images, and sources

Open **Tools & preferences** above the input field.

- Import PDF, Word, Excel, text, Markdown, CSV, JSON, logs, or supported source files. Parsing happens locally. Scanned PDFs require an installed vision model for OCR.
- Extracted text is stored under `data/documents/`. Search uses imported documents, local keyword matching, and source excerpts. You can select an individual document. Only relevant excerpts are supplied to the model.
- **Sources** show filenames, excerpt numbers, and the supplied text. PDF page markers remain in stored text.
- Discuss PNG, JPEG, and WebP attachments with an installed vision model. GLM-OCR focuses on text recognition; other compatible vision models may support broader image questions.
- **Search online (Wikipedia)** sends the query to Wikipedia and includes retrieved excerpts in the prompt. Source buttons open the page in a browser. This is encyclopedia search rather than general news search.

## Speech and preferences

- **Dictate** uses offline Windows speech recognition for up to 15 seconds. The selected recognition language must be installed on the PC.
- **Read aloud** uses an installed Windows voice. No online audio service is required.
- Choose a response style and language. Preferences and favorites are stored with conversations in `data/chats.json`. **Reset preferences** restores defaults.

## Conversations and performance

- Search conversation titles and message text in the sidebar.
- Assign a folder name to a conversation; custom names are supported.
- **Retry** starts a new conversation from the original question using the current model. The original conversation is preserved.
- **Calculate storage usage** reports model, conversation, document, backup, and WebView-profile storage without deleting files.
- During generation, the app shows elapsed time, model memory, and free RAM. Completed responses show tokens and average tokens per second, including model loading and source retrieval.

## Development checks

- [Native Cargo tests](DEVELOPMENT.md#local-checks): Rust tests for inactivity policy, document sources, hardware data, and vision-projector verification.
- `tests/workspace-ui.mjs`: browser integration using controlled API responses and installed Edge. Initialize `scripts/usb-environment.ps1` before starting Node.
- `scripts/build-ui.ps1`, `scripts/build-desktop.ps1`, and `scripts/deploy-desktop.ps1`: build and deploy to the USB, backing up previous executables.

Source changes require rebuilding the web bundle and desktop application. Run the packaging scripts afterward to include those changes in portable distributions and installers.
