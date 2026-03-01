import { invoke } from '@tauri-apps/api/core'
import { type Event, listen } from '@tauri-apps/api/event'
import { derived, writable } from 'svelte/store'

// Declare Tauri internals on window
declare global {
	interface Window {
		__TAURI_INTERNALS__?: any
	}
}

// Check if running in Tauri context
function isTauriContext(): boolean {
	return typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined
}

// Types
export interface PotentiometerData {
	pot1: number
	pot2: number
	pot3: number
}

export interface ConnectionStatus {
	connected: boolean
	port: string | null
	error: string | null
}

export interface AudioSession {
	process_id: number
	process_name: string
	display_name: string
	volume: number
	is_muted: boolean
}

export interface MixerChannel {
	id: number
	value: number
	is_physical: boolean
}

export interface SerialPortInfo {
	port_name: string
	description: string
}

// Stores
export const potentiometerData = writable<PotentiometerData>({
	pot1: 0,
	pot2: 0,
	pot3: 0,
})

export const connectionStatus = writable<ConnectionStatus>({
	connected: false,
	port: null,
	error: null,
})

export const mixerChannels = writable<MixerChannel[]>([])
export const availablePorts = writable<SerialPortInfo[]>([])
export const audioSessions = writable<AudioSession[]>([])

// Derived stores
export const channelValues = derived(
	[potentiometerData, mixerChannels],
	([$potData, $channels]) => {
		return $channels.map(channel => {
			if (channel.is_physical) {
				// Get actual pot value
				const potKey = `pot${channel.id}` as keyof PotentiometerData
				const rawValue = $potData[potKey] || 0
				const percentage = (rawValue / 4095) * 100
				// Round to nearest 2%
				const roundedValue = Math.round(percentage / 2) * 2
				return {
					...channel,
					value: roundedValue,
				}
			}
			return channel
		})
	}
)

// Initialize event listeners
export async function initializeListeners() {
	// Listen for potentiometer data
	await listen<PotentiometerData>('pot-data', (event: Event<PotentiometerData>) => {
		potentiometerData.set(event.payload)
	})

	// Listen for connection status changes
	await listen<ConnectionStatus>('connection-status', (event: Event<ConnectionStatus>) => {
		connectionStatus.set(event.payload)
	})

	// Listen for audio session updates
	await listen<AudioSession[]>('audio-sessions-updated', (event: Event<AudioSession[]>) => {
		try {
			if (event.payload && Array.isArray(event.payload)) {
				audioSessions.set(event.payload)
			} else {
				console.error('Invalid audio sessions data received:', event.payload)
			}
		} catch (error) {
			console.error('Error handling audio-sessions-updated event:', error)
		}
	})
}

// API Functions
export async function listSerialPorts(): Promise<SerialPortInfo[]> {
	try {
		const ports = await invoke<SerialPortInfo[]>('list_serial_ports')
		availablePorts.set(ports)
		return ports
	} catch (error) {
		console.error('Failed to list serial ports:', error)
		return []
	}
}

export async function connectSerial(port?: string): Promise<ConnectionStatus> {
	try {
		const status = await invoke<ConnectionStatus>('connect_serial', { port })
		connectionStatus.set(status)
		return status
	} catch (error) {
		console.error('Failed to connect serial:', error)
		const status = {
			connected: false,
			port: null,
			error: error as string,
		}
		connectionStatus.set(status)
		return status
	}
}

export async function disconnectSerial(): Promise<void> {
	try {
		await invoke('disconnect_serial')
		connectionStatus.set({
			connected: false,
			port: null,
			error: null,
		})
	} catch (error) {
		console.error('Failed to disconnect serial:', error)
	}
}

export async function setMasterVolume(volume: number): Promise<void> {
	try {
		await invoke('set_master_volume', { volume })
	} catch (error) {
		console.error('Failed to set master volume:', error)
	}
}

export async function loadMixerChannels(): Promise<MixerChannel[]> {
	try {
		const channels = await invoke<MixerChannel[]>('get_mixer_channels')
		mixerChannels.set(channels)
		return channels
	} catch (error) {
		console.error('Failed to load mixer channels:', error)
		return []
	}
}

export async function getAudioSessions(): Promise<AudioSession[]> {
	try {
		const sessions = await invoke<AudioSession[]>('get_audio_sessions')
		audioSessions.set(sessions)
		return sessions
	} catch (error) {
		console.error('Failed to get audio sessions:', error)
		return []
	}
}

// Wait for Tauri to be ready
async function waitForTauri(maxRetries = 50, retryDelay = 100): Promise<void> {
	for (let i = 0; i < maxRetries; i++) {
		if (isTauriContext()) {
			console.log(`Tauri context ready after ${i} attempts`)
			return
		}
		await new Promise(resolve => setTimeout(resolve, retryDelay))
	}
	throw new Error('Tauri context not available after maximum retries')
}

// Initialize the mixer on app start
export async function initializeMixer() {
	console.log('Starting mixer initialization...')

	try {
		// Wait for Tauri to be ready
		console.log('Waiting for Tauri context...')
		await waitForTauri()
		console.log('Tauri context is ready')

		console.log('Initializing listeners...')
		await initializeListeners()
		console.log('Listeners initialized')

		console.log('Loading mixer channels...')
		await loadMixerChannels()
		console.log('Mixer channels loaded')

		console.log('Listing serial ports...')
		await listSerialPorts()
		console.log('Serial ports listed')

		console.log('Getting audio sessions...')
		await getAudioSessions()
		console.log('Audio sessions loaded')

		// Try auto-connect but don't wait for it
		connectSerial().catch(err => {
			console.log('Auto-connect failed (this is normal if no device is connected):', err)
		})

		console.log('Mixer initialization complete!')
	} catch (error) {
		console.error('Failed to initialize mixer:', error)
		throw error
	}
}
