# GitHub Actions and issue tracking

## Continuous integration

The `CI` workflow runs on pushes to `main`, pull requests, and manual dispatches. It has two independent Windows Server 2022 jobs:

- **Web build and browser tests:** installs locked npm dependencies, checks TypeScript, builds the desktop web bundle, tests streaming, and runs the workspace and Opex Work suites in the runner's Microsoft Edge.
- **Windows native tests and build:** selects Rust 1.97.1 MSVC, initializes clean portable configuration, restores locked Cargo dependencies, and runs native tests and the desktop release build offline.

Both jobs use the project's existing scripts. Downloads and generated files remain under the checkout. Caches contain npm downloads, Cargo registry data, and compiled artifacts. Action dependencies are pinned to verified commit hashes. The token has read-only repository-content access, and checkout credentials are not persisted.

CI uses mocked browser responses and isolated native test fixtures. The two native tests requiring a live Qwen service or Hugging Face access remain explicitly ignored. No model weights, personal conversations, API keys, or training candidates are required. CI does not build installers, publish releases, or deploy the app.

After these files are committed and pushed, open the repository's Actions tab to inspect the first run. Hosted-runner execution cannot be confirmed by local tests alone. For branch protection, use the check names `Web build and browser tests` and `Windows native tests and build` after they first appear.

## Issues

The New issue page offers bug-report and feature-request forms. Bug reports request version, installation type, Windows/hardware details, reproduction steps, expected behavior, actual behavior, and redacted evidence. Feature requests ask for the problem, proposed behavior, and acceptance criteria. Blank issues remain available.

The [improvement backlog](ISSUE-BACKLOG.md) contains ready-to-file issue drafts. These drafts are not live GitHub issues. Search existing issues before filing a draft to avoid duplicates.

Suggested triage: confirm reproducibility, identify the affected version, and prioritize data preservation and broken core flows before enhancements. Add labels through GitHub as needed; the forms do not depend on pre-existing labels.

References: [GitHub issue-form syntax](https://docs.github.com/en/communities/using-templates-to-encourage-useful-issues-and-pull-requests/syntax-for-issue-forms), [Windows runner software](https://github.com/actions/runner-images/blob/main/images/windows/Windows2022-Readme.md).
