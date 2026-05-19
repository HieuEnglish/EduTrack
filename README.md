# 📚✨ EduTrack

> 🧑‍🏫 A local-first desktop app for teachers to **plan, teach, track, and report** — all on their own machine.

![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Desktop-0ea5e9?style=for-the-badge)
![Stack](https://img.shields.io/badge/Stack-Tauri%20%2B%20TypeScript-14b8a6?style=for-the-badge)
![Data](https://img.shields.io/badge/Data-SQLite%20(Local)-f59e0b?style=for-the-badge)

---

## 🚀 TL;DR (Fast Start)

```powershell
# From repo root
.\run.bat
```

That launches EduTrack in desktop mode with an AVG-safer Rust build path.

---

## 🧠 What Is EduTrack?

EduTrack is a **Tauri + Vanilla TypeScript** desktop app for classroom operations.

- 🔒 Your data stays local (SQLite on your machine)
- 🧭 Teachers get one workspace for attendance, planning, tests, and reports
- 🤖 Built-in class agents automatically surface risks and signals
- 🌐 Internet is optional (only needed for external APIs / LLM providers)

---

## ✨ Core Features

| Area | Highlights |
|---|---|
| 📊 **Overview** | School/class metrics, live classroom signals |
| 🏫 **Classrooms** | Student roster + attendance cycle (`P → L → A → E → I`) |
| 📖 **Syllabus** | Upload syllabus docs and extract editable curriculum units |
| 📝 **Lesson Plans** | Backward-design planning by week/month/year + export |
| 📅 **Calendar** | Schedule visibility + holiday syncing |
| 🤖 **Agents** | 6 always-on agents for attendance, pacing, performance, and more |
| ✏️ **Tests** | Create tests, collect submissions, auto-score, export CSV |
| 📋 **Reports** | Generate student reports using attendance + assessment data |
| ⚙️ **Settings** | Teacher profile, LLM provider config, history cleanup |

---

## 🤖 Agent System

When a class is selected, these agents run automatically:

- 🎯 `Attendance` — detects high absence trends
- ⏱️ `Lesson Pacing` — flags schedule drift / overdue planned lessons
- 📉 `Academic Performance` — highlights score drops
- ✅ `Assignment Completion` — spots completion/record mismatches
- ❤️ `Engagement Pulse` — monitors completion and lateness patterns
- 📝 `Report Writer` — detects missing or stale reports

---

## 🛠️ Tech Stack

| Layer | Technology |
|---|---|
| 🧱 Desktop shell | `Tauri v2` (Rust) |
| 🖥️ Frontend | `Vanilla TypeScript + Vite` |
| 🗄️ Database | `SQLite` via `rusqlite` (bundled) |
| 🎨 Styling | Custom CSS design system |
| 🤖 LLM (optional) | Ollama or OpenCode CLI integration |
| 📤 Export | html2pdf + CSV/JSON/TXT helpers |

---

## 📦 Project Structure

```text
EduTrack/
├── frontend/                 # TypeScript + Vite UI
│   ├── src/
│   │   ├── main.ts           # Frontend app logic
│   │   └── style.css         # Design tokens + styling
│   └── package.json
├── src-tauri/                # Rust + Tauri backend
│   ├── src/
│   │   ├── main.rs           # Tauri entrypoint + command wiring
│   │   ├── db/schema.sql     # SQLite schema source of truth
│   │   └── services/         # Attendance, agents, planning, reports, etc.
│   └── tauri.conf.json
├── scripts/
│   └── run-dev.ps1           # Launcher used by run.bat
├── Planning docs/            # Architecture/planning references
└── run.bat                   # Recommended local launcher
```

---

## ✅ Prerequisites

Install these first:

- `Node.js` 18+
- `Rust` + `Cargo`
- `Tauri CLI` (`cargo install tauri-cli`)

Optional (for richer document/media extraction in grading flows):

- `tesseract` (OCR)
- `whisper` (audio transcription)
- `pandoc` (DOC/DOCX extraction)

---

## 🧪 Local Development

### 1) Recommended desktop mode (Windows)

```powershell
.\run.bat
```

### 2) Doctor mode (if antivirus/file locks happen)

```powershell
.\run.bat -Doctor
```

### 3) Browser-only UI mode

```powershell
.\run.bat -Browser
```

### 4) Manual mode (if you want separate terminals)

```bash
# Terminal 1
cd frontend
npm install
npm run dev

# Terminal 2
cd src-tauri
cargo tauri dev --no-watch
```

---

## 🛡️ AVG-Friendly Notes (Windows)

EduTrack’s launcher uses a stable Cargo target path:

```text
D:\EduTrack\.dev-cache\cargo-target
```

If AVG blocks builds/executables, add exceptions for:

- `D:\EduTrack`
- `D:\EduTrack\.dev-cache`
- `D:\EduTrack\.dev-cache\cargo-target`
- `D:\EduTrack\src-tauri`
- `D:\EduTrack\frontend\node_modules`
- `D:\EduTrack\.dev-cache\cargo-target\debug\tauri-app.exe`

---

## 🏗️ Production Build

```bash
cd src-tauri
cargo tauri build
```

---

## 🎬 Screenshots & GIFs

Use this section on GitHub to make the project fun and easy to scan.

### 🖼️ Screenshot Gallery

> Replace these placeholders with real images from `Assets/screenshots/`.

![Dashboard Overview](Assets/screenshots/dashboard-overview.png)
![Classroom Attendance](Assets/screenshots/classroom-attendance.png)
![Lesson Planning](Assets/screenshots/lesson-planning.png)

### 🎥 Quick GIF Demos

> Replace these placeholders with short demo GIFs from `Assets/gifs/`.

![Taking Attendance](Assets/gifs/attendance-cycle.gif)
![Generating Report](Assets/gifs/report-generation.gif)

### 📸 Capture Tips

- Keep GIFs under 10-15 seconds.
- Prefer 1280x720 or 1440x900 captures.
- Focus each GIF on one task only.
- Remove personal/student data before recording.

---

## 🤝 Contributing

Contributions are welcome. Keep changes focused, test locally, and open small reviewable PRs.

### 🌿 Branch Workflow

1. `main` stays stable and releasable.
2. Create feature branches from `main`.
3. Use clear branch names:
   - `feature/<short-topic>`
   - `fix/<bug-topic>`
   - `docs/<doc-topic>`
   - `chore/<maintenance-topic>`

### ✅ Local Contribution Flow

```bash
# 1) Sync latest
git checkout main
git pull origin main

# 2) Create branch
git checkout -b feature/improve-readme

# 3) Commit your changes
git add .
git commit -m "docs: improve README structure and visuals"

# 4) Push and open PR
git push -u origin feature/improve-readme
```

### 🔍 PR Checklist

- [ ] Change is scoped and readable
- [ ] README/docs updated if behavior changed
- [ ] No secrets or local artifacts committed
- [ ] App runs locally (`.\\run.bat` on Windows)

---

## 🚀 GitHub Release Template

Use the ready template in `.github/RELEASE_TEMPLATE.md` when publishing a release.

Quick flow:

```bash
# Example
git tag v0.1.0
git push origin v0.1.0
```

Then open GitHub Releases and paste the template content.

---

## 📄 License

MIT ✅

Use it, improve it, and ship great classroom workflows. 🎓
