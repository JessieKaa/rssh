<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import * as app from "../stores/app.svelte.ts";
  import type { FrpcConfig } from "../stores/app.svelte.ts";
  import { toast } from "../stores/toast.svelte.ts";
  import { t, errMsg } from "../i18n/index.svelte.ts";

  let { id = null }: { id?: string | null } = $props();

  let config = $state<FrpcConfig | null>(null);
  let name = $state("");
  let content = $state("");
  let saving = $state(false);
  let running = $state(false);
  let logs = $state<string[]>([]);
  let showLogs = $state(true);
  let pollTimer = $state<ReturnType<typeof setInterval> | null>(null);

  onMount(async () => {
    if (id) {
      try {
        config = await invoke<FrpcConfig>("get_frpc_config", { id });
        name = config.name;
        content = await invoke<string>("read_frpc_toml", { fileName: config.file_name });
        const status = await invoke<{ running: boolean }>("frpc_status", { configId: id });
        running = status.running;
        if (running) startPolling();
      } catch (e: any) {
        toast.error(errMsg(e));
      }
    }
  });

  onDestroy(() => {
    stopPolling();
  });

  function startPolling() {
    if (pollTimer) return;
    pollTimer = setInterval(async () => {
      if (!id) return;
      try {
        const status = await invoke<{ running: boolean }>("frpc_status", { configId: id });
        running = status.running;
        if (running) {
          logs = await invoke<string[]>("frpc_logs", { configId: id, limit: 100 });
        } else {
          stopPolling();
        }
      } catch { /* ignore */ }
    }, 2000);
  }

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  async function save() {
    if (!name.trim()) {
      toast.error(t("frpc.name_empty"));
      return;
    }
    saving = true;
    try {
      if (config) {
        // Update existing
        await invoke("write_frpc_toml", { fileName: config.file_name, content });
        if (name !== config.name) {
          await invoke("rename_frpc_config", { id: config.id, newName: name });
        }
        toast.success(t("frpc.save_success"));
      } else {
        // Create new
        const newConfig = await invoke<FrpcConfig>("create_frpc_config", { name: name.trim() });
        await invoke("write_frpc_toml", { fileName: newConfig.file_name, content });
        config = newConfig;
        toast.success(t("frpc.create_success"));
      }
    } catch (e: any) {
      toast.error(errMsg(e));
    } finally {
      saving = false;
    }
  }

  async function startFrp() {
    if (!config) return;
    try {
      await invoke("frpc_start", { configId: config.id });
      running = true;
      startPolling();
    } catch (e: any) {
      toast.error(errMsg(e));
    }
  }

  async function stopFrp() {
    if (!config) return;
    try {
      await invoke("frpc_stop", { configId: config.id });
      running = false;
      stopPolling();
    } catch (e: any) {
      toast.error(errMsg(e));
    }
  }
</script>

<div class="editor-page">
  <div class="editor-toolbar">
    <button class="btn btn-sm" onclick={() => app.settingsBack()}>
      &larr; {t("common.back")}
    </button>
    <div class="toolbar-actions">
      {#if config}
        {#if running}
          <button class="btn btn-sm btn-danger" onclick={stopFrp}>
            {t("frpc.stop")}
          </button>
        {:else}
          <button class="btn btn-sm btn-accent" onclick={startFrp}>
            {t("frpc.start")}
          </button>
        {/if}
      {/if}
      <button class="btn btn-sm btn-accent" onclick={save} disabled={saving}>
        {saving ? "..." : t("frpc.save")}
      </button>
    </div>
  </div>

  {#if running}
    <div class="running-banner">
      {t("frpc.running_edit_warning")}
    </div>
  {/if}

  <div class="editor-fields">
    <label class="field">
      <span class="field-label">{t("frpc.name")}</span>
      <input
        type="text"
        bind:value={name}
        placeholder="my-server"
        spellcheck="false"
      />
    </label>
  </div>

  <div class="editor-main">
    <textarea
      bind:value={content}
      spellcheck="false"
      autocapitalize="off"
      autocomplete="off"
      placeholder={t("frpc.config_content")}
    ></textarea>
  </div>

  {#if config && showLogs}
    <div class="log-panel">
      <div class="log-header">
        <span class="log-title">{t("frpc.log.title")}</span>
        <span class="log-status" class:running>
          {running ? t("frpc.status.running") : t("frpc.status.stopped")}
        </span>
      </div>
      <div class="log-content">
        {#if logs.length === 0}
          <span class="log-empty">{t("frpc.log.empty")}</span>
        {:else}
          {#each logs as line}
            <div class="log-line">{line}</div>
          {/each}
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .editor-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 16px;
    gap: 12px;
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }
  .editor-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-shrink: 0;
  }
  .toolbar-actions { display: flex; gap: 8px; }
  .running-banner {
    padding: 8px 12px;
    background: var(--warning-bg, #fef3c7);
    color: var(--warning-text, #92400e);
    border-radius: var(--radius-sm);
    font-size: 13px;
    flex-shrink: 0;
  }
  .editor-fields {
    flex-shrink: 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field-label {
    font-size: 12px;
    color: var(--text-sub);
    font-weight: 600;
  }
  .field input {
    padding: 8px 12px;
    border: 1px solid var(--divider);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text);
    font-family: inherit;
    font-size: 14px;
  }
  .editor-main {
    flex: 1;
    min-height: 200px;
    display: flex;
  }
  .editor-main textarea {
    width: 100%;
    height: 100%;
    padding: 12px;
    border: 1px solid var(--divider);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text);
    font-family: "SF Mono", "Fira Code", "Cascadia Code", monospace;
    font-size: 13px;
    line-height: 1.5;
    resize: vertical;
    tab-size: 2;
  }
  .editor-main textarea:focus {
    outline: none;
    border-color: var(--accent);
  }
  .log-panel {
    flex-shrink: 0;
    border: 1px solid var(--divider);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }
  .log-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 12px;
    background: var(--raised);
    font-size: 12px;
    font-weight: 600;
  }
  .log-title { color: var(--text-sub); }
  .log-status { color: var(--text-dim); }
  .log-status.running { color: #4ade80; }
  .log-content {
    max-height: 200px;
    overflow-y: auto;
    padding: 8px 12px;
    font-family: monospace;
    font-size: 12px;
    line-height: 1.6;
    background: var(--bg);
  }
  .log-line { white-space: pre-wrap; word-break: break-all; }
  .log-empty { color: var(--text-dim); font-style: italic; }
</style>
