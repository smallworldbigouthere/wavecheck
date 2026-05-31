# wavecheck

A free, clean desktop app for downloading audio and video from playlists and links —
a friendly front-end for the open-source tools that do the heavy lifting (**yt-dlp** + **FFmpeg**).

- Paste a playlist or video URL → pick **Audio (MP3)** or **Video (MP4)** → choose a folder → download.
- Live per-track progress, auto-resume (skips what you already have).
- Self-updates its download engine so it keeps working as sites change.
- macOS + Windows.

## Download

Grab the latest installer from the [**Releases**](../../releases/latest) page:
- **macOS** — `.dmg`
- **Windows** — `.exe` / `.msi`

### ⚠️ First launch: opening an unsigned app

wavecheck isn't code-signed yet, so your OS will warn you the first time (this is normal —
it just means the developer identity isn't registered; the app is unchanged and safe). You
only have to do this **once**:

**macOS**
1. Double-click the app. You'll see *"wavecheck can't be opened because it is from an unidentified developer."*
2. **Right-click (or Control-click) the app → Open → Open.**
3. After that first time, it opens normally — and updates normally.
   - If it's still blocked: **System Settings → Privacy & Security**, scroll down, and click **Open Anyway**.

**Windows**
1. If you see *"Windows protected your PC"* (SmartScreen):
2. Click **More info → Run anyway.**
3. Done — subsequent launches open without the prompt.

## Cookies / "Sign in to confirm you're not a bot"

Some sites gate downloads behind a sign-in check. wavecheck can use your **browser's existing
login** to get past it — pick your browser (Chrome, Safari, etc.) in the app. Your cookies stay
on your machine and are never uploaded.

## It's free — please support the projects that power it

wavecheck takes **no money** and shows **no ads**. It's a front-end; the real work is done by:

- **yt-dlp** — the download engine — https://github.com/yt-dlp/yt-dlp ([support the maintainers](https://github.com/yt-dlp/yt-dlp/blob/master/Maintainers.md))
- **FFmpeg** — audio/video processing — https://ffmpeg.org ([donate](https://ffmpeg.org/donations.html))

If wavecheck is useful to you, please donate to **them**. See [`THIRD-PARTY-NOTICES.md`](src-tauri/licenses/THIRD-PARTY-NOTICES.md) for licenses.

## License

wavecheck is free to use; see [`LICENSE`](LICENSE). Bundled components (yt-dlp, FFmpeg)
remain under their own licenses.

---

### Building from source (developers)

```bash
npm install
node scripts/fetch-sidecars.mjs   # downloads yt-dlp + ffmpeg for your OS
npm run tauri dev                 # run locally
npm run build:clean               # produce a release build with no local paths embedded
```
