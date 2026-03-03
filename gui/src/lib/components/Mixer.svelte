<script lang="ts">
	import { channelValues } from '$lib/stores/mixer'
	import Fader from './Fader.svelte'

	let channels = $channelValues

	$: channels = $channelValues
</script>

<div class="mixer-container">
	<h2 class="mixer-title">Audio Mixer</h2>

	<div class="faders-container">
		{#each channels as channel (channel.id)}
			<Fader {channel} value={channel.value} />
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

	.mixer-title {
		text-align: center;
		color: rgba(255, 255, 255, 0.95);
		margin-bottom: 20px;
		font-size: 20px;
		font-weight: 400;
		letter-spacing: 1px;
		text-transform: uppercase;
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
