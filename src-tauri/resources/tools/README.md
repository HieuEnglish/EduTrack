Bundled CLI Tools

EduTrack can ship optional local binaries for:
- `tesseract`
- `pandoc`

Place binaries in this folder structure before running a release bundle:

- Windows:
  - `resources/tools/windows/tesseract/tesseract.exe`
  - `resources/tools/windows/pandoc/pandoc.exe`
- macOS:
  - `resources/tools/macos/tesseract/tesseract`
  - `resources/tools/macos/pandoc/pandoc`
- Linux:
  - `resources/tools/linux/tesseract/tesseract`
  - `resources/tools/linux/pandoc/pandoc`

Runtime resolution order:
1. Explicit override env vars:
   - `EDUTRACK_TESSERACT_PATH`
   - `EDUTRACK_PANDOC_PATH`
2. Bundled binary under `resources/tools/<platform>/...`
3. System `PATH`

Notes:
- Keep licenses for bundled third-party tools in your release artifacts.
- Ensure macOS/Linux binaries are executable (`chmod +x`).
