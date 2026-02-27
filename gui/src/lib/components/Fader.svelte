<script lang="ts">
    import type { MixerChannel } from "$lib/stores/mixer";

    export let channel: MixerChannel;
    export let value: number = 0;
    export let onVolumeChange: (value: number) => void = () => {};
    export let onDrop: (event: DragEvent) => void = () => {};
    export let onClear: () => void = () => {};

    let isDraggingOver = false;

    function handleSliderChange(event: Event) {
        const target = event.target as HTMLInputElement;
        const newValue = parseFloat(target.value);
        onVolumeChange(newValue);
    }

    function handleDragOver(event: DragEvent) {
        event.preventDefault();
        isDraggingOver = true;
    }

    function handleDragLeave() {
        isDraggingOver = false;
    }

    function handleDrop(event: DragEvent) {
        event.preventDefault();
        isDraggingOver = false;
        onDrop(event);
    }

    $: faderHeight = 100 - value;
</script>

<div
    class="fader-container"
    class:dragging-over={isDraggingOver}
    on:dragover={handleDragOver}
    on:dragleave={handleDragLeave}
    on:drop={handleDrop}
    role="region"
    aria-label="Channel {channel.id} fader"
>
    <div class="fader-header">
        <span class="channel-number">CH {channel.id}</span>
        {#if channel.is_physical}
            <span class="channel-type">Physical</span>
        {:else}
            <span class="channel-type">Virtual</span>
        {/if}
    </div>

    <div class="fader-body">
        <div class="volume-display">{Math.round(value)}%</div>

        <div class="fader-track">
            <div class="fader-fill" style="height: {100 - faderHeight}%"></div>
            <input
                type="range"
                min="0"
                max="100"
                step="1"
                {value}
                on:input={handleSliderChange}
                class="fader-slider"
                disabled={channel.is_physical}
            />
        </div>

        <div class="scale-marks">
            <div class="mark" style="bottom: 100%">100</div>
            <div class="mark" style="bottom: 75%">75</div>
            <div class="mark" style="bottom: 50%">50</div>
            <div class="mark" style="bottom: 25%">25</div>
            <div class="mark" style="bottom: 0%">0</div>
        </div>
    </div>

    <div class="fader-footer">
        {#if channel.mapped_app}
            <div class="mapped-app">
                <span class="app-name">{channel.mapped_app}</span>
                {#if !channel.is_physical}
                    <button class="clear-btn" on:click={onClear}>×</button>
                {/if}
            </div>
        {:else if !channel.is_physical}
            <div class="drop-zone">Drop app here</div>
        {:else}
            <div class="pot-indicator">
                Pot {channel.id}
            </div>
        {/if}
    </div>
</div>

<style>
    .fader-container {
        display: flex;
        flex-direction: column;
        width: 80px;
        height: 400px;
        background: #2a2a2a;
        border-radius: 8px;
        padding: 10px;
        margin: 0 5px;
        box-shadow: 0 2px 10px rgba(0, 0, 0, 0.3);
        transition: all 0.3s ease;
    }

    .fader-container.dragging-over {
        background: #3a3a3a;
        border: 2px solid #4a9eff;
    }

    .fader-header {
        display: flex;
        flex-direction: column;
        align-items: center;
        margin-bottom: 10px;
    }

    .channel-number {
        font-weight: bold;
        font-size: 14px;
        color: #fff;
    }

    .channel-type {
        font-size: 10px;
        color: #888;
        text-transform: uppercase;
    }

    .fader-body {
        flex: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        position: relative;
    }

    .volume-display {
        font-size: 16px;
        font-weight: bold;
        color: #4a9eff;
        margin-bottom: 10px;
    }

    .fader-track {
        position: relative;
        width: 40px;
        height: 200px;
        background: #1a1a1a;
        border-radius: 20px;
        overflow: hidden;
        box-shadow: inset 0 2px 5px rgba(0, 0, 0, 0.5);
    }

    .fader-fill {
        position: absolute;
        bottom: 0;
        left: 0;
        right: 0;
        background: linear-gradient(to top, #4a9eff, #6ab7ff);
        border-radius: 20px;
        transition: height 0.1s ease;
    }

    .fader-slider {
        position: absolute;
        width: 200px;
        height: 40px;
        left: 50%;
        top: 50%;
        transform: translate(-50%, -50%) rotate(-90deg);
        opacity: 0;
        cursor: pointer;
    }

    .fader-slider:disabled {
        cursor: default;
    }

    .scale-marks {
        position: absolute;
        right: -25px;
        top: 0;
        bottom: 0;
        width: 20px;
    }

    .mark {
        position: absolute;
        font-size: 9px;
        color: #666;
        transform: translateY(50%);
    }

    .fader-footer {
        height: 40px;
        display: flex;
        align-items: center;
        justify-content: center;
        margin-top: 10px;
    }

    .mapped-app {
        display: flex;
        align-items: center;
        gap: 5px;
        padding: 5px 10px;
        background: #1a1a1a;
        border-radius: 4px;
        width: 100%;
    }

    .app-name {
        font-size: 11px;
        color: #fff;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        flex: 1;
    }

    .clear-btn {
        background: #ff4444;
        color: white;
        border: none;
        border-radius: 50%;
        width: 16px;
        height: 16px;
        font-size: 12px;
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        transition: background 0.2s;
    }

    .clear-btn:hover {
        background: #ff6666;
    }

    .drop-zone {
        padding: 8px;
        border: 2px dashed #444;
        border-radius: 4px;
        font-size: 10px;
        color: #666;
        text-align: center;
        width: 100%;
    }

    .pot-indicator {
        font-size: 11px;
        color: #4a9eff;
        text-align: center;
    }
</style>

