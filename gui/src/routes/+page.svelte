<script lang="ts">
	import { onMount } from "svelte";
	import { initializeMixer } from "$lib/stores/mixer";
	import StatusBar from "$lib/components/StatusBar.svelte";
	import Mixer from "$lib/components/Mixer.svelte";
	import ApplicationList from "$lib/components/ApplicationList.svelte";

	let isInitialized = false;
	let initError: string | null = null;

	onMount(async () => {
		try {
			console.log('Page mounted, initializing mixer...');
			await initializeMixer();
			isInitialized = true;
			console.log('Page initialization complete');
		} catch (error) {
			console.error('Failed to initialize:', error);
			initError = error instanceof Error ? error.message : String(error);
			// Still set initialized to true so we can show the error
			isInitialized = true;
		}
	});
</script>

<main class="app">
	<header class="app-header">
		<h1>🎛️ PC Audio Mixer</h1>
		<p class="subtitle">Control your audio with physical potentiometers</p>
	</header>

	<div class="app-content">
		{#if isInitialized}
			{#if initError}
				<div class="error">
					<h2>Initialization Error</h2>
					<p>{initError}</p>
					<p class="hint">Check the browser console for more details</p>
				</div>
			{:else}
				<StatusBar />
				<div class="main-layout">
					<div class="mixer-section"><Mixer /></div>
					<div class="sidebar"><ApplicationList /></div>
				</div>
			{/if}
		{:else}
			<div class="loading">
				<div class="spinner"></div>
				<p>Initializing mixer...</p>
			</div>
		{/if}
	</div>
</main>

<style>
	:global(body) {
		margin: 0;
		padding: 0;
		font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen,
			Ubuntu, Cantarell, "Open Sans", "Helvetica Neue", sans-serif;
		background: linear-gradient(135deg, #1a1a1a, #2d2d2d);
		color: #f0f0f0;
		min-height: 100vh;
	}

	.app {
		min-height: 100vh;
		display: flex;
		flex-direction: column;
	}

	.app-header {
		padding: 20px;
		text-align: center;
		background: rgba(0, 0, 0, 0.3);
		border-bottom: 1px solid rgba(255, 255, 255, 0.1);
	}

	.app-header h1 {
		margin: 0;
		font-size: 28px;
		font-weight: 300;
		letter-spacing: 2px;
	}

	.subtitle {
		margin: 5px 0 0;
		color: #888;
		font-size: 14px;
	}

	.app-content {
		flex: 1;
		padding: 20px;
		max-width: 1200px;
		margin: 0 auto;
		width: 100%;
	}

	.loading {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 400px;
		color: #888;
	}

	.spinner {
		width: 50px;
		height: 50px;
		border: 3px solid rgba(255, 255, 255, 0.1);
		border-top-color: #4a9eff;
		border-radius: 50%;
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.loading p {
		margin-top: 20px;
		font-size: 14px;
	}

	.error {
		text-align: center;
		padding: 40px;
		color: #ff6b6b;
	}

	.error h2 {
		margin-bottom: 16px;
		font-size: 24px;
	}

	.error p {
		margin: 8px 0;
	}

	.error .hint {
		color: #888;
		font-size: 14px;
		margin-top: 20px;
	}

	.main-layout {
		display: grid;
		grid-template-columns: 1fr 380px;
		gap: 20px;
		margin-top: 20px;
	}

	.mixer-section {
		min-width: 0;
	}

	.sidebar {
		min-width: 0;
	}

	@media (max-width: 1000px) {
		.main-layout {
			grid-template-columns: 1fr;
		}

		.sidebar {
			margin-top: 20px;
		}
	}
</style>
