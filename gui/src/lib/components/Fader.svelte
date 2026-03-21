<script lang="ts">
	import { setPotMapping, draggedApp, type MixerChannel } from '$lib/stores/mixer'

	let { channel, value = 0, mappedApp = null }: {
		channel: MixerChannel
		value?: number
		mappedApp?: string | null
	} = $props()

	let faderHeight = $derived(100 - value)

	let dragEnterCount = $state(0)
	let isDragOver = $derived(dragEnterCount > 0)

	let dropSuccess = $state(false)
	let dropTimer: ReturnType<typeof setTimeout> | null = null

	function handleDragEnter(event: DragEvent) {
		dragEnterCount++
		event.preventDefault()
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'move'
		}
	}

	function handleDragLeave() {
		dragEnterCount = Math.max(0, dragEnterCount - 1)
	}

	function handleDragOver(event: DragEvent) {
		event.preventDefault()
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'move'
		}
	}

	async function handleDrop(event: DragEvent) {
		event.preventDefault()
		dragEnterCount = 0

		// Use shared store as primary source (reliable across all webviews),
		// fall back to dataTransfer for external drag compatibility
		const processName = $draggedApp || event.dataTransfer?.getData('text/plain')
		if (!processName) return

		try {
			await setPotMapping(channel.id, processName)

			if (dropTimer) clearTimeout(dropTimer)
			dropSuccess = true
			dropTimer = setTimeout(() => {
				dropSuccess = false
				dropTimer = null
			}, 400)
		} catch (err) {
			console.error('Failed to set pot mapping on drop:', err)
		}
	}

	async function handleClear() {
		try {
			await setPotMapping(channel.id, null)
		} catch (err) {
			console.error('Failed to clear pot mapping:', err)
		}
	}

	$effect(() => {
		return () => {
			if (dropTimer) clearTimeout(dropTimer)
		}
	})
</script>

<div
	class="fader-container"
	class:drag-over={isDragOver}
	class:drop-success={dropSuccess}
	role="region"
	aria-label="Channel {channel.id} fader"
	ondragenter={handleDragEnter}
	ondragleave={handleDragLeave}
	ondragover={handleDragOver}
	ondrop={handleDrop}
>
	<div class="fader-header"><span class="channel-number">CH {channel.id}</span></div>

	<div class="fader-body">
		<div class="volume-display">{Math.round(value)}%</div>

		<div class="fader-track">
			<div class="fader-fill" style="height: {100 - faderHeight}%"></div>
			<input type="range" min="0" max="100" step="1" {value} class="fader-slider" disabled>
		</div>
	</div>

	<div class="fader-footer">
		<div class="pot-indicator">Pot {channel.id}</div>
		{#if mappedApp}
			<div class="mapped-app-row">
				<div class="mapped-app" title={mappedApp}>
					{mappedApp.length > 10 ? mappedApp.substring(0, 10) + '...' : mappedApp}
				</div>
				<button class="clear-btn" onclick={handleClear} title="Remove mapping" aria-label="Remove mapping">&times;</button>
			</div>
		{/if}
	</div>
</div>

<style>
	.fader-container {
		display: flex;
		flex-direction: column;
		width: 60px;
		height: 320px;
		background: transparent;
		padding: 7px;
		margin: 0 8px;
		border: 1px solid transparent;
		border-radius: 8px;
	}

	.fader-header {
		display: flex;
		flex-direction: column;
		align-items: center;
		margin-bottom: 8px;
	}

	.channel-number {
		font-weight: 500;
		font-size: 12px;
		color: rgba(255, 255, 255, 0.9);
	}

	.fader-body {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		position: relative;
		padding: 0 4px;
	}

	.volume-display {
		font-size: 13px;
		font-weight: 500;
		color: rgba(255, 255, 255, 0.85);
		margin-bottom: 8px;
	}

	.fader-track {
		position: relative;
		width: 6px;
		height: 200px;
		background: rgba(255, 255, 255, 0.08);
		border-radius: 3px;
		overflow: visible;
	}

	.fader-fill {
		position: absolute;
		bottom: 0;
		left: 0;
		right: 0;
		background: rgba(255, 255, 255, 0.95);
		border-radius: 3px;
		transition: height 0.1s ease;
		box-shadow: 0 0 8px rgba(255, 255, 255, 0.3);
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

	.fader-footer {
		min-height: 30px;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		margin-top: 8px;
		gap: 2px;
	}

	.pot-indicator {
		font-size: 10px;
		color: rgba(255, 255, 255, 0.6);
		text-align: center;
	}

	.fader-container.drag-over {
		border-color: rgba(100, 180, 255, 0.6);
		box-shadow: 0 0 12px rgba(100, 180, 255, 0.3);
	}

	.fader-container.drop-success {
		animation: drop-pulse 0.4s ease;
	}

	@keyframes drop-pulse {
		0% {
			border-color: rgba(80, 220, 120, 0.8);
			box-shadow: 0 0 16px rgba(80, 220, 120, 0.4);
		}
		100% {
			border-color: transparent;
			box-shadow: none;
		}
	}

	.mapped-app-row {
		display: flex;
		align-items: center;
		gap: 2px;
		justify-content: center;
	}

	.mapped-app {
		font-size: 9px;
		color: rgba(100, 180, 255, 0.8);
		text-align: center;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: 48px;
	}

	.clear-btn {
		background: rgba(255, 80, 80, 0.3);
		color: rgba(255, 255, 255, 0.8);
		border: none;
		border-radius: 50%;
		width: 14px;
		height: 14px;
		font-size: 9px;
		line-height: 1;
		padding: 0;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.clear-btn:hover {
		background: rgba(255, 80, 80, 0.6);
	}
</style>
