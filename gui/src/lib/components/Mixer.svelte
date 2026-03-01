<script lang="ts">
	import { channelValues } from '$lib/stores/mixer'
	import Fader from './Fader.svelte'

	let _channels = $channelValues

	$: _channels = $channelValues
</script>

<div class="mixer-container">
	<h2 class="mixer-title">Audio Mixer</h2>

	<div class="faders-container">
		{#each _channels as channel (channel.id)}
			<Fader {channel} value={channel.value} />
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
