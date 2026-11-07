# Orvyn improvement backlog

These are issue drafts for review, not published GitHub issues. Priorities are proposed. Check existing issues before filing them.

## 1. Rebuild both 0.7.0 installers from the current source

**Priority:** High

The current installer EXEs reuse an older desktop executable. The current source includes the model library, English text, application license notices, and a repaired native build.

**Acceptance criteria**

- Build the web bundle and native executable from the same recorded commit.
- Package standard and without-Orvyn-1 installers from that build.
- Verify the standard package contains the expected model and notices; the other contains no bundled weights or Orvyn 1 profile.
- Generate SHA-256 files and record the source commit in the release notes.
- Both fresh installations open successfully.

## 2. Verify installer upgrades preserve user data for both variants

**Priority:** High

The existing installer test assumes one bundled model. Extend it to cover both variants and upgrades using synthetic fixtures.

**Acceptance criteria**

- Fresh-install, upgrade, and uninstall tests cover both installer variants.
- Upgrades preserve fixture conversations, API configuration, custom models, and backups.
- Uninstall removes application files while preserving documented user data.
- A source-repository or drive-root destination is rejected.
- Tests never use personal conversations or real API credentials.

## 3. Add a clear first-launch flow without a bundled model

**Priority:** Medium

Users installing without Orvyn 1 need an obvious way to begin.

**Acceptance criteria**

- With no usable model or API connection, show Download a model and Connect an API actions.
- Each action opens the relevant existing screen.
- Explain when a network connection is needed and display download size when known.
- Dismissing the flow leaves the workspace usable.
- Existing installations with a configured model retain their normal startup behavior.

## 4. Explain model compatibility and memory requirements before download

**Priority:** Medium

GGUF availability alone does not guarantee compatibility with the bundled runtime or a user's hardware.

**Acceptance criteria**

- Show download size and estimated RAM before starting a download.
- Identify required vision-projector files and include their sizes.
- Distinguish verified compatibility from unknown compatibility.
- Explain that RAM figures are estimates and preserve the user's ability to choose.
- Cover missing metadata and insufficient-memory guidance in UI tests.

## 5. Add an opt-in release-packaging workflow

**Priority:** Medium

The initial CI checks source builds. Installer packaging requires separately supplied model weights and inference runtime files.

**Acceptance criteria**

- Define trusted sources, pinned versions, checksums, and required notices for runtime and model inputs.
- Run source CI before packaging.
- Build both variants and publish checksummed artifacts from a recorded commit.
- Require explicit release intent before publication; pull requests do not publish.
- Keep release credentials restricted to the publishing job.
- Exclude user configuration, conversations, and API keys from artifacts.
