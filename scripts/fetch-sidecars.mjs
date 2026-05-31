#!/usr/bin/env node
// Download the yt-dlp + ffmpeg + ffprobe sidecars for the *current* platform and
// name them with the Rust target triple Tauri expects (binaries/<name>-<triple>[.exe]).
//
// Run locally or in CI before `tauri build`. Keeps large binaries out of git.

import { writeFileSync, mkdirSync, chmodSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const binDir = join(here, "..", "src-tauri", "binaries");
mkdirSync(binDir, { recursive: true });

const platform = process.platform; // 'darwin' | 'win32' | 'linux'
const arch = process.arch; // 'arm64' | 'x64'

const TRIPLE = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
  "win32-x64": "x86_64-pc-windows-msvc",
  "linux-x64": "x86_64-unknown-linux-gnu",
}[`${platform}-${arch}`];

if (!TRIPLE) throw new Error(`Unsupported platform: ${platform}-${arch}`);
const exe = platform === "win32" ? ".exe" : "";

// yt-dlp standalone (macOS binary is a universal2 build, fine for both arches).
const YTDLP = {
  darwin: "yt-dlp_macos",
  win32: "yt-dlp.exe",
  linux: "yt-dlp_linux",
}[platform];

// ffmpeg/ffprobe static builds from eugeneware/ffmpeg-static (GPL).
const FF_SUFFIX = {
  "darwin-arm64": "darwin-arm64",
  "darwin-x64": "darwin-x64",
  "win32-x64": "win32-x64.exe",
  "linux-x64": "linux-x64",
}[`${platform}-${arch}`];

const downloads = [
  {
    url: `https://github.com/yt-dlp/yt-dlp/releases/latest/download/${YTDLP}`,
    out: `yt-dlp-${TRIPLE}${exe}`,
  },
  {
    url: `https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffmpeg-${FF_SUFFIX}`,
    out: `ffmpeg-${TRIPLE}${exe}`,
  },
  {
    url: `https://github.com/eugeneware/ffmpeg-static/releases/latest/download/ffprobe-${FF_SUFFIX}`,
    out: `ffprobe-${TRIPLE}${exe}`,
  },
];

for (const { url, out } of downloads) {
  process.stdout.write(`Fetching ${out} … `);
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) throw new Error(`HTTP ${res.status} for ${url}`);
  const buf = Buffer.from(await res.arrayBuffer());
  const dest = join(binDir, out);
  writeFileSync(dest, buf);
  if (platform !== "win32") chmodSync(dest, 0o755);
  console.log(`${(buf.length / 1048576).toFixed(1)} MB`);
}

console.log(`✓ sidecars ready for ${TRIPLE} in src-tauri/binaries/`);
