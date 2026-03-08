<script lang="ts">
	import { channelValues, connectionStatus, potMappings } from '$lib/stores/mixer'
	import Fader from './Fader.svelte'

	let channels = $derived($channelValues)
	let status = $derived($connectionStatus)

	let mappedApps = $derived(
		Object.fromEntries(
			$potMappings.map((m) => [m.pot_index, m.process_name])
		) as Record<number, string>
	)
</script>

<div class="mixer-container">
	<div class="mixer-header">
		<h2 class="mixer-title">Audio Mixer</h2>
		<span
			class="connection-dot"
			class:connected={status.connected}
			title={status.connected ? `Connected to ${status.port}` : 'Disconnected'}
		></span>
	</div>

	<div class="faders-container">
		{#each channels as channel (channel.id)}
			<Fader {channel} value={channel.value} mappedApp={mappedApps[channel.id] ?? null} />
		{/each}
	</div>
</div>

<style>
	.mixer-container {
		display: flex;
		flex-direction: column;
		padding: 20px;
		background: rgba(30, 30, 30, 0.6);
		backdrop-filter: blur(20px);
		border-radius: 12px;
		box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
	}

	.mixer-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 20px;
	}

	.mixer-title {
		text-align: center;
		color: rgba(255, 255, 255, 0.95);
		margin: 0;
		font-size: 20px;
		font-weight: 400;
		letter-spacing: 1px;
		text-transform: uppercase;
		flex: 1;
	}

	.connection-dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		background: #ff4444;
		box-shadow: 0 0 6px rgba(255, 68, 68, 0.5);
		animation: pulse-dot 2s infinite;
		flex-shrink: 0;
	}

	.connection-dot.connected {
		background: #44ff44;
		box-shadow: 0 0 8px rgba(68, 255, 68, 0.6);
		animation: none;
	}

	@keyframes pulse-dot {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.4; }
	}

	.faders-container {
		display: flex;
		justify-content: center;
		gap: 10px;
		padding: 20px;
		background: rgba(40, 40, 40, 0.4);
		border-radius: 8px;
		overflow-x: auto;
	}

	@media (max-width: 1200px) {
		.faders-container {
			flex-wrap: wrap;
		}
	}

	@media (max-width: 768px) {
		.faders-container {
			justify-content: flex-start;
		}
	}
</style>
