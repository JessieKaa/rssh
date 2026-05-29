<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import * as app from "../stores/app.svelte.ts";
  import type { FrpcConfig } from "../stores/app.svelte.ts";
  import { toast } from "../stores/toast.svelte.ts";
  import { t, errMsg } from "../i18n/index.svelte.ts";

  let items = $state<FrpcConfig[]>([]);
  let statuses = $state<Record<string, boolean>>({});
  let loading = $state(true);
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  onMount(async () => {
    await reload();
    pollTimer = setInterval(pollStatus, 3000);
  });

  onDestroy(() => {
    if (pollTimer) { clearInterval(pollTimer); pollTimer = null; }
  });

  async function pollStatus() {
    if (items.length === 0) return;
    try {
      const entries: [string, boolean][] = await Promise.all(
        items.map(async (c) => {
          try {
            const s = await invoke<{ running: boolean }>("frpc_status", { configId: c.id });
            return [c.id, s.running] as [string, boolean];
          } catch {
            return [c.id, false] as [string, boolean];
          }
        })
      );
      statuses = Object.fromEntries(entries);
    } catch { /* ignore */ }
  }

  async function reload() {
    loading = true;
    try {
      items = await app.loadFrpcConfigs();
      await pollStatus();
    } catch (e: any) {
      toast.error(`${t("toast.error.load")}: ${errMsg(e)}`);
    } finally {
      loading = false;
    }
  }

  let deleting = $state<string | null>(null);
  async function remove(id: string) {
    if (!confirm(t("frpc.confirm_delete"))) return;
    deleting = id;
    try {
      await invoke("delete_frpc_config", { id });
      await reload();
    } catch (e: any) {
      toast.error(`${t("toast.error.delete")}: ${errMsg(e)}`);
    } finally {
      deleting = null;
    }
  }

  async function toggleEnabled(config: FrpcConfig) {
    try {
      await invoke("toggle_frpc_enabled", { configId: config.id, enabled: !config.enabled });
      config.enabled = !config.enabled;
      items = [...items]; // trigger reactivity
    } catch (e: any) {
      toast.error(errMsg(e));
    }
  }

  let starting = $state<string | null>(null);
  async function start(id: string) {
    starting = id;
    try {
      await invoke("frpc_start", { configId: id });
      await pollStatus();
    } catch (e: any) {
      toast.error(errMsg(e));
    } finally {
      starting = null;
    }
  }

  let stopping = $state<string | null>(null);
  async function stop(id: string) {
    stopping = id;
    try {
      await invoke("frpc_stop", { configId: id });
      await pollStatus();
    } catch (e: any) {
      toast.error(errMsg(e));
    } finally {
      stopping = null;
    }
  }
</script>

<div class="page">
  <div class="toolbar">
    <button class="btn btn-accent btn-sm" onclick={() => app.navigate("frpc-edit")}>
      {t("frpc.new")}
    </button>
  </div>

  {#if loading}
    <p class="empty">{t("common.loading")}</p>
  {:else if items.length === 0}
    <p class="empty">{t("frpc.empty")}</p>
  {:else}
    {#each items as c (c.id)}
      <div class="card item-row">
        <div class="item-info">
          <div class="item-name">
            <span class="status-dot" class:running={statuses[c.id]}></span>
            {c.name}
          </div>
          <div class="item-sub">{c.file_name}</div>
        </div>
        <div class="item-actions">
          <label class="toggle-label" title={t("frpc.enabled")}>
            <input
              type="checkbox"
              checked={c.enabled}
              onchange={() => toggleEnabled(c)}
            />
            <span class="toggle-text">{t("frpc.enabled")}</span>
          </label>
          {#if statuses[c.id]}
            <button
              class="btn btn-sm btn-danger"
              onclick={() => stop(c.id)}
              disabled={stopping === c.id}
            >
              {stopping === c.id ? "..." : t("frpc.stop")}
            </button>
          {:else}
            <button
              class="btn btn-sm btn-accent"
              onclick={() => start(c.id)}
              disabled={starting === c.id}
            >
              {starting === c.id ? "..." : t("frpc.start")}
            </button>
          {/if}
          <button class="btn btn-sm" onclick={() => app.navigate("frpc-edit", c.id)}>
            {t("common.edit")}
          </button>
          <button
            class="btn btn-sm btn-danger"
            onclick={() => remove(c.id)}
            disabled={deleting === c.id}
          >
            {deleting === c.id ? "..." : t("common.delete")}
          </button>
        </div>
      </div>
    {/each}
  {/if}
</div>

<style>
  .page { padding: 24px; }
  .toolbar { display: flex; justify-content: flex-end; margin-bottom: 16px; }
  .item-row { display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px; }
  .item-name { font-weight: 600; font-size: 14px; display: flex; align-items: center; gap: 8px; }
  .item-sub { font-size: 12px; color: var(--text-sub); font-family: monospace; }
  .item-actions { display: flex; gap: 10px; align-items: center; }
  .empty { text-align: center; color: var(--text-dim); padding: 32px; }
  .status-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-dim);
  }
  .status-dot.running { background: #4ade80; }
  .toggle-label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--text-sub);
    cursor: pointer;
  }
  .toggle-text { white-space: nowrap; }
</style>
