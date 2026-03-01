<script lang="ts">
	import {
		type ConnectionStatus,
		connectSerial,
		disconnectSerial,
		listSerialPorts,
		type SerialPortInfo,
		connectionStatus,
		availablePorts,
	} from "$lib/stores/mixer";

	let status: ConnectionStatus = { connected: false, port: null, error: null };
	let ports: SerialPortInfo[] = [];
	let selectedPort: string = "";
	let showPortSelector = false;
	let isConnecting = false;

	$: status = $connectionStatus;
	$: ports = $availablePorts;

	async function handleConnect() {
		isConnecting = true;
		try {
			await connectSerial(selectedPort || undefined);
		} finally {
			isConnecting = false;
			showPortSelector = false;
		}
	}

	async function handleDisconnect() {
		await disconnectSerial();
	}

	async function togglePortSelector() {
		if (!showPortSelector) {
			await listSerialPorts();
		}
		showPortSelector = !showPortSelector;
	}

	async function handleReconnect() {
		await handleDisconnect();
		setTimeout(() => {
			handleConnect();
		}, 500);
	}
</script>

<div class="status-bar">
	<div class="status-indicator" class:connected={status.connected}>
		<span class="status-dot"></span>
		<span class="status-text">
			{#if status.connected}
				Connected to {status.port}
			{:else if isConnecting}
				Connecting...
			{:else}
				Disconnected
			{/if}
		</span>
	</div>

	<div class="status-controls">
		{#if status.connected}
			<button
				type="button"
				class="btn btn-disconnect"
				on:click={handleDisconnect}
			>
				Disconnect
			</button>
			<button type="button" class="btn btn-reconnect" on:click={handleReconnect}
				>Reconnect</button
			>
		{:else}
			<button
				type="button"
				class="btn btn-connect"
				on:click={handleConnect}
				disabled={isConnecting}
			>
				{isConnecting ? "Connecting..." : "Auto Connect"}
			</button>
			<button
				type="button"
				class="btn btn-select"
				on:click={togglePortSelector}
			>
				Select Port
			</button>
		{/if}
	</div>

	{#if showPortSelector && !status.connected}
		<div class="port-selector">
			<h4>Available Ports:</h4>
			{#if ports.length > 0}
				<select bind:value={selectedPort} class="port-select">
					<option value="">Auto-detect</option>
					{#each ports as port}
						<option value={port.port_name}
							>{port.port_name} - {port.description}</option
						>
					{/each}
				</select>
				<button
					type="button"
					class="btn btn-primary"
					on:click={handleConnect}
					disabled={isConnecting}
				>
					Connect to Selected
				</button>
			{:else}
				<p class="no-ports">No serial ports detected</p>
				<button type="button" class="btn" on:click={listSerialPorts}
					>Refresh</button
				>
			{/if}
		</div>
	{/if}

	{#if status.error}
		<div class="error-message">⚠️ {status.error}</div>
	{/if}
</div>

<style>
	.status-bar {
		background: #2a2a2a;
		border-radius: 8px;
		padding: 15px;
		margin-bottom: 20px;
	}

	.status-indicator {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-bottom: 10px;
	}

	.status-dot {
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background: #ff4444;
		box-shadow: 0 0 10px rgba(255, 68, 68, 0.5);
		animation: pulse 2s infinite;
	}

	.status-indicator.connected .status-dot {
		background: #44ff44;
		box-shadow: 0 0 10px rgba(68, 255, 68, 0.5);
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.5;
		}
	}

	.status-text {
		color: #fff;
		font-size: 14px;
	}

	.status-controls {
		display: flex;
		gap: 10px;
		margin-top: 10px;
	}

	.btn {
		padding: 8px 16px;
		border: none;
		border-radius: 4px;
		font-size: 13px;
		cursor: pointer;
		transition: all 0.2s;
		background: #3a3a3a;
		color: #fff;
	}

	.btn:hover {
		background: #4a4a4a;
	}

	.btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.btn-connect {
		background: #4a9eff;
	}

	.btn-connect:hover {
		background: #6ab7ff;
	}

	.btn-disconnect {
		background: #ff4444;
	}

	.btn-disconnect:hover {
		background: #ff6666;
	}

	.btn-reconnect {
		background: #ff9800;
	}

	.btn-reconnect:hover {
		background: #ffb74d;
	}

	.btn-primary {
		background: #4a9eff;
	}

	.btn-primary:hover {
		background: #6ab7ff;
	}

	.port-selector {
		margin-top: 15px;
		padding: 15px;
		background: #1e1e1e;
		border-radius: 6px;
	}

	.port-selector h4 {
		color: #fff;
		margin-bottom: 10px;
		font-size: 14px;
	}

	.port-select {
		width: 100%;
		padding: 8px;
		background: #2a2a2a;
		color: #fff;
		border: 1px solid #444;
		border-radius: 4px;
		margin-bottom: 10px;
	}

	.no-ports {
		color: #888;
		font-size: 13px;
		margin: 10px 0;
	}

	.error-message {
		margin-top: 10px;
		padding: 10px;
		background: rgba(255, 68, 68, 0.1);
		border: 1px solid rgba(255, 68, 68, 0.3);
		border-radius: 4px;
		color: #ff6666;
		font-size: 13px;
	}
</style>
