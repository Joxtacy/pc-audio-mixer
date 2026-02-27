<script lang="ts">
    import { audioSessions, type AudioSession } from '$lib/stores/mixer';

    export let onDragStart: (app: any) => void = () => {};

    let sessions: AudioSession[] = [];

    $: sessions = $audioSessions;

    function handleDragStart(session: AudioSession, event: DragEvent) {
        const appData = {
            process_id: session.process_id,
            process_name: session.process_name,
            is_master: false
        };

        if (event.dataTransfer) {
            event.dataTransfer.effectAllowed = 'copy';
            event.dataTransfer.setData('application/json', JSON.stringify(appData));
        }

        onDragStart(appData);
    }

    function handleMasterDragStart(event: DragEvent) {
        const masterData = {
            process_id: null,
            process_name: 'Master Volume',
            is_master: true
        };

        if (event.dataTransfer) {
            event.dataTransfer.effectAllowed = 'copy';
            event.dataTransfer.setData('application/json', JSON.stringify(masterData));
        }

        onDragStart(masterData);
    }
</script>

<div class="app-selector">
    <h3>Available Applications</h3>

    <div class="app-list">
        <!-- Master Volume -->
        <div
            class="app-item master"
            draggable="true"
            on:dragstart={handleMasterDragStart}
            role="button"
            tabindex="0"
        >
            <div class="app-icon">🔊</div>
            <div class="app-info">
                <div class="app-name">Master Volume</div>
                <div class="app-description">System Master</div>
            </div>
            <div class="volume-indicator">
                <span class="drag-hint">Drag to assign</span>
            </div>
        </div>

        <!-- Application Sessions -->
        {#each sessions as session (session.process_id)}
            <div
                class="app-item"
                draggable="true"
                on:dragstart={(e) => handleDragStart(session, e)}
                role="button"
                tabindex="0"
            >
                <div class="app-icon">🎵</div>
                <div class="app-info">
                    <div class="app-name">{session.display_name}</div>
                    <div class="app-description">{session.process_name}</div>
                </div>
                <div class="volume-indicator">
                    <div class="volume-bar">
                        <div
                            class="volume-fill"
                            style="width: {session.volume}%"
                        ></div>
                    </div>
                    <span class="volume-text">{Math.round(session.volume)}%</span>
                </div>
            </div>
        {/each}

        {#if sessions.length === 0}
            <div class="no-apps">
                No audio applications detected.
                <br />
                Start playing audio in an app to see it here.
            </div>
        {/if}
    </div>
</div>

<style>
    .app-selector {
        background: #2a2a2a;
        border-radius: 8px;
        padding: 15px;
        margin-bottom: 20px;
    }

    h3 {
        color: #fff;
        margin-bottom: 15px;
        font-size: 16px;
        font-weight: 400;
    }

    .app-list {
        display: flex;
        flex-direction: column;
        gap: 8px;
    }

    .app-item {
        display: flex;
        align-items: center;
        padding: 10px;
        background: #1e1e1e;
        border-radius: 6px;
        cursor: grab;
        transition: all 0.2s;
    }

    .app-item:hover {
        background: #252525;
        transform: translateX(2px);
    }

    .app-item:active {
        cursor: grabbing;
        opacity: 0.8;
    }

    .app-item.master {
        background: linear-gradient(135deg, #1e1e1e, #2a2a2a);
        border: 1px solid #444;
    }

    .app-icon {
        font-size: 24px;
        margin-right: 12px;
    }

    .app-info {
        flex: 1;
    }

    .app-name {
        color: #fff;
        font-size: 14px;
        font-weight: 500;
    }

    .app-description {
        color: #888;
        font-size: 11px;
        margin-top: 2px;
    }

    .volume-indicator {
        display: flex;
        align-items: center;
        gap: 10px;
    }

    .volume-bar {
        width: 60px;
        height: 4px;
        background: #1a1a1a;
        border-radius: 2px;
        overflow: hidden;
    }

    .volume-fill {
        height: 100%;
        background: #4a9eff;
        transition: width 0.2s;
    }

    .volume-text {
        color: #888;
        font-size: 11px;
        min-width: 30px;
        text-align: right;
    }

    .drag-hint {
        color: #666;
        font-size: 11px;
        font-style: italic;
    }

    .no-apps {
        text-align: center;
        color: #666;
        padding: 20px;
        font-size: 13px;
        line-height: 1.5;
    }
</style>