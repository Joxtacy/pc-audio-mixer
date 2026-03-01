<script lang="ts">
    import { audioSessions } from '$lib/stores/mixer';
    import type { AudioSession } from '$lib/stores/mixer';

    function getVolumeIcon(volume: number): string {
        if (volume === 0) return '🔇';
        if (volume < 33) return '🔈';
        if (volume < 66) return '🔉';
        return '🔊';
    }

    function formatProcessName(name: string): string {
        // Sanitize and remove common file extensions
        const sanitized = name
            .replace(/[<>]/g, '') // Remove potential HTML tags
            .replace(/\.(exe|app)$/i, ''); // Remove file extensions
        return sanitized.substring(0, 100); // Limit length
    }

    function sanitizeDisplayName(name: string): string {
        // Sanitize display name to prevent XSS
        return name
            .replace(/[<>]/g, '') // Remove potential HTML tags
            .substring(0, 100); // Limit length
    }
</script>

<div class="application-list">
    <h3>Available Audio Applications</h3>

    {#if $audioSessions.length === 0}
        <div class="no-apps">No audio applications detected</div>
    {:else}
        <ul class="app-list">
            {#each $audioSessions as session (session.process_id)}
                <li class="app-item" class:master={session.process_id === 0}>
                    <div class="app-info">
                        <span class="app-name">
                            {#if session.process_id === 0}
                                <span class="master-icon">🎚️</span>
                            {/if}
                            {sanitizeDisplayName(session.display_name)}
                        </span>
                        <span class="process-name">
                            {formatProcessName(session.process_name)}
                        </span>
                    </div>

                    <div class="volume-info">
                        <span class="volume-icon">
                            {getVolumeIcon(session.volume)}
                        </span>
                        <span class="volume-value">
                            {Math.round(session.volume)}%
                        </span>
                        {#if session.is_muted}
                            <span class="muted-indicator">MUTED</span>
                        {/if}
                    </div>
                </li>
            {/each}
        </ul>
    {/if}
</div>

<style>
    .application-list {
        background: rgba(30, 30, 30, 0.8);
        border-radius: 12px;
        padding: 20px;
        backdrop-filter: blur(10px);
    }

    h3 {
        margin: 0 0 16px 0;
        color: #fff;
        font-size: 18px;
        font-weight: 600;
        letter-spacing: 0.5px;
    }

    .no-apps {
        color: #888;
        text-align: center;
        padding: 20px;
        font-style: italic;
    }

    .app-list {
        list-style: none;
        padding: 0;
        margin: 0;
    }

    .app-item {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 12px 16px;
        margin-bottom: 8px;
        background: rgba(45, 45, 45, 0.6);
        border-radius: 8px;
        transition: all 0.2s ease;
        border: 1px solid rgba(255, 255, 255, 0.1);
    }

    .app-item:hover {
        background: rgba(55, 55, 55, 0.8);
        border-color: rgba(255, 255, 255, 0.2);
        transform: translateX(2px);
    }

    .app-item.master {
        background: linear-gradient(135deg, rgba(100, 60, 200, 0.2), rgba(60, 100, 200, 0.2));
        border-color: rgba(100, 100, 255, 0.3);
    }

    .app-item.master:hover {
        background: linear-gradient(135deg, rgba(100, 60, 200, 0.3), rgba(60, 100, 200, 0.3));
        border-color: rgba(100, 100, 255, 0.5);
    }

    .app-info {
        display: flex;
        flex-direction: column;
        gap: 4px;
        flex: 1;
    }

    .app-name {
        color: #fff;
        font-size: 14px;
        font-weight: 500;
        display: flex;
        align-items: center;
        gap: 8px;
    }

    .master-icon {
        font-size: 16px;
    }

    .process-name {
        color: #888;
        font-size: 11px;
        font-family: 'Courier New', monospace;
    }

    .volume-info {
        display: flex;
        align-items: center;
        gap: 8px;
    }

    .volume-icon {
        font-size: 16px;
        opacity: 0.8;
    }

    .volume-value {
        color: #bbb;
        font-size: 13px;
        font-weight: 500;
        min-width: 40px;
        text-align: right;
    }

    .muted-indicator {
        background: rgba(255, 50, 50, 0.2);
        color: #ff6666;
        padding: 2px 6px;
        border-radius: 4px;
        font-size: 10px;
        font-weight: 600;
        letter-spacing: 0.5px;
    }
</style>