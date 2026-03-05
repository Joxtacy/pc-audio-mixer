<script lang="ts">
	import type { MixerChannel } from '$lib/stores/mixer'

	export let channel: MixerChannel
	export let value: number = 0
	export let mappedApp: string | null = null

	$: faderHeight = 100 - value
</script>

<div class="fader-container" role="region" aria-label="Channel {channel.id} fader">
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
			<div class="mapped-app" title={mappedApp}>
				{mappedApp.length > 10 ? mappedApp.substring(0, 10) + '...' : mappedApp}
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
		padding: 8px;
		margin: 0 8px;
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

	.mapped-app {
		font-size: 9px;
		color: rgba(100, 180, 255, 0.8);
		text-align: center;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: 60px;
	}
</style>
