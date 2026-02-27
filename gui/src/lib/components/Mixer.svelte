<script lang="ts">
    import { onMount } from 'svelte';
    import {
        channelValues,
        saveChannelMapping,
        clearChannelMapping,
        setAppVolume,
        setMasterVolume,
        type ChannelMapping
    } from '$lib/stores/mixer';
    import Fader from './Fader.svelte';

    let channels = $channelValues;
    let draggedApp: any = null;

    $: channels = $channelValues;

    async function handleVolumeChange(channelId: number, value: number) {
        const channel = channels.find(ch => ch.id === channelId);
        if (!channel || channel.is_physical) return;

        // Update virtual channel volume
        if (channel.mapped_app) {
            if (channel.app_process_id) {
                await setAppVolume(channel.app_process_id, value);
            }
        }
    }

    async function handleDrop(channelId: number, event: DragEvent) {
        if (!draggedApp) return;

        const mapping: ChannelMapping = {
            channel_id: channelId,
            process_id: draggedApp.process_id,
            process_name: draggedApp.process_name,
            is_master: draggedApp.is_master || false
        };

        await saveChannelMapping(mapping);
        draggedApp = null;
    }

    async function handleClear(channelId: number) {
        await clearChannelMapping(channelId);
    }

    // Set dragged app from external source (AppSelector component)
    function setDraggedApp(app: any) {
        draggedApp = app;
    }

    // Export this function so parent components can use it
    export { setDraggedApp };
</script>

<div class="mixer-container">
    <h2 class="mixer-title">Audio Mixer</h2>

    <div class="faders-container">
        {#each channels as channel (channel.id)}
            <Fader
                {channel}
                value={channel.value}
                onVolumeChange={(value) => handleVolumeChange(channel.id, value)}
                onDrop={(event) => handleDrop(channel.id, event)}
                onClear={() => handleClear(channel.id)}
            />
        {/each}
    </div>
</div>

<style>
    .mixer-container {
        display: flex;
        flex-direction: column;
        padding: 20px;
        background: #1e1e1e;
        border-radius: 12px;
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.5);
    }

    .mixer-title {
        text-align: center;
        color: #fff;
        margin-bottom: 20px;
        font-size: 24px;
        font-weight: 300;
        letter-spacing: 2px;
        text-transform: uppercase;
    }

    .faders-container {
        display: flex;
        justify-content: center;
        gap: 10px;
        padding: 20px;
        background: #252525;
        border-radius: 8px;
    }

    @media (max-width: 768px) {
        .faders-container {
            flex-wrap: wrap;
        }
    }
</style>