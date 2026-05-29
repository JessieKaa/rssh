<script lang="ts">
    import * as app from "../stores/app.svelte.ts";
    import * as ai from "../ai/store.svelte.ts";
    import { toast } from "../stores/toast.svelte.ts";
    import { t } from "../i18n/index.svelte.ts";
    import { onDestroy } from "svelte";

    function prevent(e: Event) { e.preventDefault(); }

    let _repeatTimer: ReturnType<typeof setInterval> | null = null;

    function repeatStart(fn: () => void) {
        repeatStop();
        fn();
        _repeatTimer = setInterval(fn, 100);
    }

    function repeatStop() {
        if (_repeatTimer) { clearInterval(_repeatTimer); _repeatTimer = null; }
    }

    onDestroy(repeatStop);

    function send(seq: string) {
        app.sendToTerminal(seq);
        app.clearModifiers();
    }

    function arrow(dir: app.ArrowDir) {
        const ctrl = app.ctrlActive();
        const alt = app.altActive();
        const mod = (ctrl && alt) ? 7 : ctrl ? 5 : alt ? 3 : 0;
        app.sendArrow(dir, mod);
        app.clearModifiers();
    }

    // 当前 tab 是否有活跃 SSH/local session——AI 面板要求已连接的终端做诊断对象。
    // 没连接就让按钮 disabled，避免点了没反应（aiVisible 在 AppShell 层会因 session 缺失静默不渲染）。
    let canOpenAi = $derived.by(() => {
        const tab = app.activeTab();
        if (!tab || (tab.type !== "ssh" && tab.type !== "local")) return false;
        return !!app.sessionIdForTab(tab.id);
    });

    // 移动端唤起 AI 时提示一次：建议横屏 + 两个工具不可用。
    // 模块级 flag——一次 app run 提一次；togglePanel 只有"开"动作时才提。
    let mobileHintShown = false;
    function toggleAi() {
        if (!ai.isOpen() && !canOpenAi) {
            toast.info(t("ai.no_session"));
            return;
        }
        // AI / SFTP 互斥：开 AI 时自动关 SFTP
        if (!ai.isOpen() && app.sftpOpen()) {
            app.closeSftp();
        }
        if (!ai.isOpen() && !mobileHintShown) {
            toast.info(t("ai.mobile.hint"));
            mobileHintShown = true;
        }
        ai.togglePanel();
    }

    let canOpenSftp = $derived.by(() => {
        const tab = app.activeTab();
        if (!tab || tab.type !== "ssh") return false;
        return !!app.sessionIdForTab(tab.id);
    });

    function toggleSftp() {
        if (!app.sftpOpen() && !canOpenSftp) return;
        // AI / SFTP 互斥：开 SFTP 时自动关 AI
        if (!app.sftpOpen() && ai.isOpen()) {
            ai.togglePanel();
        }
        if (app.sftpOpen()) {
            app.closeSftp();
        } else {
            app.openSftp();
        }
    }
</script>

<div class="keybar">
    <button class="key mod" class:active={app.ctrlActive()} onpointerdown={prevent} onclick={() => app.setCtrl(!app.ctrlActive())}>Ctrl</button>
    <button class="key mod" class:active={app.altActive()} onpointerdown={prevent} onclick={() => app.setAlt(!app.altActive())}>Alt</button>
    <button class="key" onpointerdown={prevent} onclick={() => send('\x1b')}>Esc</button>
    <button class="key" onpointerdown={prevent} onclick={() => send('\t')}>Tab</button>
    <button class="key" onpointerdown={(e) => { prevent(e); repeatStart(() => arrow('A')); }} onpointerup={repeatStop} onpointerleave={repeatStop} onpointercancel={repeatStop}>↑</button>
    <button class="key" onpointerdown={(e) => { prevent(e); repeatStart(() => arrow('B')); }} onpointerup={repeatStop} onpointerleave={repeatStop} onpointercancel={repeatStop}>↓</button>
    <button class="key" onpointerdown={(e) => { prevent(e); repeatStart(() => arrow('D')); }} onpointerup={repeatStop} onpointerleave={repeatStop} onpointercancel={repeatStop}>←</button>
    <button class="key" onpointerdown={(e) => { prevent(e); repeatStart(() => arrow('C')); }} onpointerup={repeatStop} onpointerleave={repeatStop} onpointercancel={repeatStop}>→</button>
    <button class="key" title="Snippets" onpointerdown={prevent} onclick={() => app.openSnippetPicker()}>⚡</button>
    <button class="key" class:active={ai.isOpen()} class:dim={!ai.isOpen() && !canOpenAi} title="AI Chat" onpointerdown={prevent} onclick={toggleAi}>AI</button>
    <button class="key" class:active={app.sftpOpen()} class:dim={!app.sftpOpen() && !canOpenSftp} title={t("tab.context.sftp")} onpointerdown={prevent} onclick={toggleSftp}>SFTP</button>
</div>

<style>
    .keybar {
        display: flex;
        gap: 4px;
        padding: 6px 8px;
        background: var(--bg);
        border-top: 1px solid var(--divider);
        flex-shrink: 0;
        overflow-x: auto;
        overflow-y: hidden;
        -webkit-overflow-scrolling: touch;
        scrollbar-width: none;
        -ms-overflow-style: none;
    }
    .keybar::-webkit-scrollbar { display: none; }
    .key {
        flex: 1 0 0;
        min-width: 42px;
        white-space: nowrap;
        height: 36px;
        border: none;
        border-radius: 6px;
        background: var(--surface);
        color: var(--text-sub);
        font-family: inherit;
        font-size: 13px;
        font-weight: 600;
        cursor: pointer;
        -webkit-tap-highlight-color: transparent;
        user-select: none;
    }
    .key:active {
        background: var(--divider);
    }
    .key.mod.active {
        background: var(--accent);
        color: var(--white);
    }
    .key.dim { opacity: 0.45; }
</style>
