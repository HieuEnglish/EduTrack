# Windows Release Signing (Required For Distribution)

Unsigned installers are often flagged by SmartScreen/AV.  
For teacher distribution, sign release artifacts before sharing.

## Prerequisites

1. A code-signing certificate installed in the Windows certificate store.
2. The certificate thumbprint.
3. `signtool.exe` (Windows SDK Signing Tools).

## 1) Build installer artifacts

```powershell
cd src-tauri
cargo tauri build --bundles msi,nsis
```

Artifacts are typically produced under:

- `src-tauri\target\release\bundle\msi\`
- `src-tauri\target\release\bundle\nsis\`

## 2) Sign artifacts

From repo root:

```powershell
.\scripts\sign-release.ps1 -CertThumbprint "<YOUR_CERT_THUMBPRINT>"
```

Optional timestamp authority override:

```powershell
.\scripts\sign-release.ps1 -CertThumbprint "<YOUR_CERT_THUMBPRINT>" -TimestampUrl "http://timestamp.digicert.com"
```

The script signs and verifies all `.msi` and `.exe` files in `src-tauri\target\release\bundle`.

## 3) Verify before publishing

1. Install from the signed installer on a clean Windows machine.
2. Confirm Publisher is your organization in the installer/security dialog.
3. Upload only signed artifacts to GitHub Releases.

## Notes

- Keep certificate private key access limited to release maintainers.
- If you automate releases in CI, store signing secrets in secure CI secret storage.
- Local lock/debug notes in this repo apply to development builds only.
