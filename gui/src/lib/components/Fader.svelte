<script lang="ts">
	import type { MixerChannel } from '$lib/stores/mixer'

	export let channel: MixerChannel
	export let value: number = 0

	$: faderHeight = 100 - value
</script>

<div class="fader-container" role="region" aria-label="Channel {channel.id} fader">
	<div class="fader-header">
		<span class="channel-number">CH {channel.id}</span>
		<span class="channel-type">Physical</span>
	</div>

	<div class="fader-body">
		<div class="volume-display">{Math.round(value)}%</div>

		<div class="fader-track">
			<div class="fader-fill" style="height: {100 - faderHeight}%"></div>
			<input type="range" min="0" max="100" step="1" {value} class="fader-slider" disabled>
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
		<div class="pot-indicator">Pot {channel.id}</div>
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

	.pot-indicator {
		font-size: 11px;
		color: #4a9eff;
		text-align: center;
	}
</style>
