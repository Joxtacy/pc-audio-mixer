import { invoke } from '@tauri-apps/api/core'
import { type Event, listen } from '@tauri-apps/api/event'
import { derived, writable } from 'svelte/store'

// Declare Tauri internals on window
declare global {
	interface Window {
		__TAURI_INTERNALS__?: unknown
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
	pot4: number
	pot5: number
	pot6: number
	pot7: number
	pot8: number
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

export interface PotMapping {
	pot_index: number
	process_name: string
}

// Stores
export const potentiometerData = writable<PotentiometerData>({
	pot1: 0,
	pot2: 0,
	pot3: 0,
	pot4: 0,
	pot5: 0,
	pot6: 0,
	pot7: 0,
	pot8: 0,
})

export const connectionStatus = writable<ConnectionStatus>({
	connected: false,
	port: null,
	error: null,
})

export const mixerChannels = writable<MixerChannel[]>([])
export const availablePorts = writable<SerialPortInfo[]>([])
export const audioSessions = writable<AudioSession[]>([])
export const potMappings = writable<PotMapping[]>([])

// Derived stores
export const channelValues = derived(
	[potentiometerData, mixerChannels],
	([$potData, $channels]) => {
		return $channels.map(channel => {
			if (channel.is_physical) {
				// Get actual pot value
				const potKey = `pot${channel.id}` as keyof PotentiometerData
				const rawValue = $potData[potKey] || 0
				const percentage = (rawValue / 1023) * 100
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

	// Listen for pot mapping updates
	await listen<PotMapping[]>('pot-mappings-updated', (event: Event<PotMapping[]>) => {
		if (event.payload && Array.isArray(event.payload)) {
			potMappings.set(event.payload)
		}
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

export async function getPotMappings(): Promise<PotMapping[]> {
	try {
		const mappings = await invoke<PotMapping[]>('get_pot_mappings')
		potMappings.set(mappings)
		return mappings
	} catch (error) {
		console.error('Failed to get pot mappings:', error)
		return []
	}
}

export async function setPotMapping(potIndex: number, processName: string | null): Promise<void> {
	try {
		const mappings = await invoke<PotMapping[]>('set_pot_mapping', { potIndex, processName })
		potMappings.set(mappings)
	} catch (error) {
		console.error('Failed to set pot mapping:', error)
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
async function waitForTauri(maxRetries = 10, retryDelay = 200): Promise<void> {
	console.log('Checking for Tauri context...')

	// Check immediately if Tauri is available
	if (isTauriContext()) {
		console.log('Tauri context ready immediately')
		return
	}

	// Try a few times with shorter delay
	for (let i = 0; i < maxRetries; i++) {
		console.log(`Waiting for Tauri context... attempt ${i + 1}/${maxRetries}`)
		await new Promise(resolve => setTimeout(resolve, retryDelay))

		if (isTauriContext()) {
			console.log(`Tauri context ready after ${i + 1} attempts`)
			return
		}
	}

	// If we're in a browser, show a helpful message
	if (typeof window !== 'undefined' && window.location.protocol.startsWith('http')) {
		console.warn('Not running in Tauri context - possibly opened in browser')
		throw new Error('This application must be run as a Tauri desktop app, not in a browser. Please run "pnpm tauri dev" and use the native window.')
	}

	// Otherwise, something is wrong with the Tauri initialization
	console.error('Tauri context not available after maximum retries. Window object:', window)
	throw new Error('Failed to initialize Tauri context. The application may not be running correctly.')
}

// Initialize the mixer on app start
export async function initializeMixer() {
	console.log('Starting mixer initialization...')
	const errors: string[] = []

	try {
		// Wait for Tauri to be ready
		console.log('Waiting for Tauri context...')
		await waitForTauri()
		console.log('Tauri context is ready')

		console.log('Initializing listeners...')
		await initializeListeners()
		console.log('Listeners initialized')

		// Load mixer channels - non-critical, continue if fails
		try {
			console.log('Loading mixer channels...')
			await loadMixerChannels()
			console.log('Mixer channels loaded')
		} catch (error) {
			console.error('Failed to load mixer channels:', error)
			errors.push(`Mixer channels: ${error}`)
		}

		// List serial ports - non-critical, continue if fails
		try {
			console.log('Listing serial ports...')
			await listSerialPorts()
			console.log('Serial ports listed')
		} catch (error) {
			console.error('Failed to list serial ports:', error)
			errors.push(`Serial ports: ${error}`)
		}

		// Get audio sessions - critical for Windows functionality
		try {
			console.log('Getting audio sessions...')
			const sessions = await getAudioSessions()
			console.log(`Audio sessions loaded: ${sessions.length} sessions found`)

			// If no sessions on Windows, that's a problem
			if (sessions.length === 0 && navigator.platform.includes('Win')) {
				console.warn('No audio sessions found on Windows - COM initialization may have failed')
			}
		} catch (error) {
			console.error('Failed to get audio sessions:', error)
			errors.push(`Audio sessions: ${error}`)

			// Set some default mock data so the UI still works
			audioSessions.set([{
				process_id: 0,
				process_name: "Master",
				display_name: "Master Volume (Error loading sessions)",
				volume: 50,
				is_muted: false
			}])
		}

		// Load saved pot mappings
		try {
			console.log('Loading pot mappings...')
			await getPotMappings()
			console.log('Pot mappings loaded')
		} catch (error) {
			console.error('Failed to load pot mappings:', error)
			errors.push(`Pot mappings: ${error}`)
		}

		// Try auto-connect but don't wait for it
		connectSerial().catch(err => {
			console.log('Auto-connect failed (this is normal if no device is connected):', err)
		})

		if (errors.length > 0) {
			console.warn('Mixer initialization completed with errors:', errors)
		} else {
			console.log('Mixer initialization complete!')
		}
	} catch (error) {
		console.error('Failed to initialize mixer:', error)
		throw error
	}
}
