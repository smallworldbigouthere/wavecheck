<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { check } from "@tauri-apps/plugin-updater";
  import { onMount, onDestroy } from "svelte";

  type Probe = { title: string; count: number; kind: string };

  // wavecheck is free and takes no cut. All donations go straight to the
  // open-source projects that make it work.
  const credits = [
    {
      name: "yt-dlp",
      what: "the download engine (a fork of youtube-dl)",
      donate: "https://github.com/yt-dlp/yt-dlp/blob/master/Maintainers.md",
    },
    {
      name: "FFmpeg",
      what: "audio/video conversion + merging",
      donate: "https://ffmpeg.org/donations.html",
    },
  ];

  type Progress = {
    index: number | null;
    total: number | null;
    percent: string;
    speed: string;
    eta: string;
    title: string;
  };

  let url = $state("");
  let browser = $state("chrome");
  let format = $state<"audio" | "video">("audio");
  let outDir = $state("");

  let probing = $state(false);
  let probe = $state<Probe | null>(null);
  let error = $state("");

  let downloading = $state(false);
  let progress = $state<Progress | null>(null);
  let log = $state<string[]>([]);
  let doneCode = $state<number | null>(null);

  let engineMsg = $state("");
  let engineBusy = $state(false);

  let appUpdate = $state<{ version: string } | null>(null);
  let appUpdating = $state(false);

  let unlisten: UnlistenFn[] = [];

  onMount(async () => {
    unlisten.push(
      await listen<Progress>("wc://progress", (e) => {
        progress = e.payload;
      }),
    );
    unlisten.push(
      await listen<string>("wc://log", (e) => {
        log = [e.payload, ...log].slice(0, 200);
      }),
    );
    unlisten.push(
      await listen<number>("wc://done", (e) => {
        downloading = false;
        doneCode = e.payload;
      }),
    );
    unlisten.push(
      await listen<{ version: string }>("wc://engine", (e) => {
        engineMsg = `engine ${e.payload.version}`;
      }),
    );

    // Check for a new wavecheck release (silent if no endpoint configured / offline).
    try {
      const upd = await check();
      if (upd) appUpdate = { version: upd.version };
    } catch {
      appUpdate = null;
    }
  });

  async function installAppUpdate() {
    appUpdating = true;
    try {
      const upd = await check();
      if (upd) {
        await upd.downloadAndInstall();
        appUpdate = null;
        alert("Update installed — please reopen wavecheck to finish.");
      }
    } catch (e) {
      alert(`Update failed: ${e}`);
    } finally {
      appUpdating = false;
    }
  }

  async function updateEngine() {
    engineBusy = true;
    try {
      const s = await invoke<{ version: string; message: string }>("update_engine");
      engineMsg = `${s.message} · ${s.version}`;
    } catch (e) {
      engineMsg = String(e);
    } finally {
      engineBusy = false;
    }
  }

  onDestroy(() => unlisten.forEach((u) => u()));

  async function doProbe() {
    error = "";
    probe = null;
    doneCode = null;
    if (!url.trim()) return;
    probing = true;
    try {
      probe = await invoke<Probe>("probe", { url, browser });
    } catch (e) {
      error = String(e);
    } finally {
      probing = false;
    }
  }

  async function pickFolder() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") outDir = dir;
  }

  async function start() {
    error = "";
    doneCode = null;
    if (!outDir) {
      error = "Choose a destination folder first.";
      return;
    }
    log = [];
    progress = null;
    downloading = true;
    try {
      await invoke("start_download", {
        opts: { url, outDir, format, browser },
      });
    } catch (e) {
      error = String(e);
      downloading = false;
    }
  }

  const pct = $derived(
    progress?.percent ? parseFloat(progress.percent.replace("%", "")) || 0 : 0,
  );
</script>

<main>
  <header>
    <h1>wavecheck</h1>
    <span class="tag">media collector</span>
    <div class="engine">
      {#if engineMsg}<span class="muted">{engineMsg}</span>{/if}
      <button class="ghost sm" onclick={updateEngine} disabled={engineBusy}>
        {engineBusy ? "Updating…" : "Update engine"}
      </button>
    </div>
  </header>

  <section class="card">
    <label class="field">
      <span>Playlist or video URL</span>
      <div class="row">
        <input
          placeholder="https://www.youtube.com/playlist?list=…"
          bind:value={url}
          onkeydown={(e) => e.key === "Enter" && doProbe()}
        />
        <button class="ghost" onclick={doProbe} disabled={probing || !url.trim()}>
          {probing ? "Checking…" : "Check"}
        </button>
      </div>
    </label>

    {#if probe}
      <div class="probe">
        <strong>{probe.title}</strong>
        <span class="muted">
          {probe.kind === "playlist"
            ? `${probe.count} track${probe.count === 1 ? "" : "s"}`
            : "single video"}
        </span>
      </div>
    {/if}

    <div class="options">
      <label class="field">
        <span>Format</span>
        <div class="seg">
          <button
            class:active={format === "audio"}
            onclick={() => (format = "audio")}>Audio · MP3</button
          >
          <button
            class:active={format === "video"}
            onclick={() => (format = "video")}>Video · MP4</button
          >
        </div>
      </label>

      <label class="field">
        <span>Sign-in cookies from</span>
        <select bind:value={browser}>
          <option value="chrome">Chrome</option>
          <option value="safari">Safari</option>
          <option value="brave">Brave</option>
          <option value="edge">Edge</option>
          <option value="firefox">Firefox</option>
          <option value="">None</option>
        </select>
      </label>
    </div>

    <label class="field">
      <span>Save to</span>
      <div class="row">
        <input readonly placeholder="Choose a folder…" value={outDir} />
        <button class="ghost" onclick={pickFolder}>Browse…</button>
      </div>
    </label>

    <button class="primary" onclick={start} disabled={downloading || !url.trim()}>
      {downloading ? "Downloading…" : "Download"}
    </button>

    {#if error}<p class="error">{error}</p>{/if}
    {#if doneCode !== null}
      <p class="done">
        {doneCode === 0
          ? "✓ Finished."
          : `Finished with skips/errors (exit ${doneCode}). Unavailable tracks were skipped.`}
      </p>
    {/if}
  </section>

  {#if downloading || progress}
    <section class="card">
      <div class="progress-head">
        {#if progress?.index && progress?.total}
          <span class="counter">{progress.index} / {progress.total}</span>
        {/if}
        <span class="track">{progress?.title || "Preparing…"}</span>
        <span class="muted">{progress?.speed} {progress?.eta}</span>
      </div>
      <div class="bar"><div class="fill" style={`width:${pct}%`}></div></div>
    </section>
  {/if}

  {#if log.length}
    <section class="card log">
      {#each log as line}<div class="line">{line}</div>{/each}
    </section>
  {/if}

  {#if appUpdate}
    <button class="update-bar" onclick={installAppUpdate} disabled={appUpdating}>
      {appUpdating
        ? "Installing update…"
        : `wavecheck ${appUpdate.version} is available — click to update`}
    </button>
  {/if}

  <footer class="credits">
    <p class="credits-head">
      <strong>wavecheck is free</strong> — and takes no cut. It's a front-end for two
      open-source projects. If it's useful to you, please support the people who actually
      make it work:
    </p>
    {#each credits as c}
      <div class="credit">
        <span><strong>{c.name}</strong> — {c.what}</span>
        <button class="link" onclick={() => openUrl(c.donate)}>Donate ↗</button>
      </div>
    {/each}
  </footer>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: Inter, -apple-system, BlinkMacSystemFont, sans-serif;
    background: #faf9f7;
    color: #1a1a1a;
  }
  :global(*) {
    box-sizing: border-box;
  }
  main {
    max-width: 640px;
    margin: 0 auto;
    padding: 28px 24px 48px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  h1 {
    font-size: 26px;
    margin: 0;
    letter-spacing: -0.02em;
  }
  .tag {
    font-size: 12px;
    background: #f3eefe;
    color: #5832a8;
    padding: 3px 8px;
    border-radius: 999px;
  }
  .engine {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sm {
    padding: 5px 10px;
    font-size: 12px;
  }
  .card {
    background: #fff;
    border: 1px solid #e8e2db;
    border-radius: 14px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.03);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field > span {
    font-size: 12px;
    font-weight: 600;
    color: #6b6357;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .row {
    display: flex;
    gap: 8px;
  }
  input,
  select {
    flex: 1;
    padding: 10px 12px;
    border: 1px solid #d3cabe;
    border-radius: 8px;
    font-size: 14px;
    background: #fff;
    color: inherit;
  }
  input:focus,
  select:focus {
    outline: none;
    border-color: #6b46c1;
  }
  button {
    cursor: pointer;
    border-radius: 8px;
    border: 1px solid transparent;
    font-size: 14px;
    font-weight: 600;
    padding: 10px 16px;
    font-family: inherit;
  }
  .ghost {
    background: #f4f1ed;
    border-color: #d3cabe;
    color: #1a1a1a;
  }
  .ghost:hover {
    background: #ece7e0;
  }
  .primary {
    background: #6b46c1;
    color: #fff;
    padding: 12px;
    font-size: 15px;
  }
  .primary:hover {
    background: #5832a8;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .options {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  .seg {
    display: flex;
    border: 1px solid #d3cabe;
    border-radius: 8px;
    overflow: hidden;
  }
  .seg button {
    flex: 1;
    border: none;
    border-radius: 0;
    background: #fff;
    color: #6b6357;
    font-weight: 500;
  }
  .seg button.active {
    background: #6b46c1;
    color: #fff;
  }
  .probe {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px;
    background: #f3eefe;
    border-radius: 8px;
  }
  .muted {
    color: #a09786;
    font-size: 13px;
  }
  .error {
    color: #c53030;
    margin: 0;
    font-size: 13px;
  }
  .done {
    color: #15803d;
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }
  .progress-head {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }
  .counter {
    font-weight: 700;
    color: #6b46c1;
  }
  .track {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bar {
    height: 8px;
    background: #ece7e0;
    border-radius: 999px;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: #6b46c1;
    transition: width 0.2s;
  }
  .log {
    max-height: 220px;
    overflow-y: auto;
    font-family: "JetBrains Mono", ui-monospace, monospace;
    font-size: 11px;
    color: #6b6357;
    gap: 2px;
  }
  .line {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .credits {
    margin-top: 4px;
    padding: 16px 18px;
    border-top: 1px solid #e8e2db;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .credits-head {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: #6b6357;
  }
  .credit {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    font-size: 13px;
    color: #1a1a1a;
  }
  .link {
    background: none;
    border: none;
    color: #6b46c1;
    padding: 4px 6px;
    font-size: 13px;
    text-decoration: underline;
  }
  .link:hover {
    color: #5832a8;
  }
  .update-bar {
    width: 100%;
    background: #f3eefe;
    color: #5832a8;
    border: 1px solid #d9cdf5;
    border-radius: 10px;
    padding: 10px;
    font-size: 13px;
  }
  .update-bar:hover {
    background: #ece1fb;
  }
</style>
