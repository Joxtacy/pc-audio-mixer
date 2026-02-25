import { writable, derived } from 'svelte/store';
import { invoke, type Event } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Types
export interface PotentiometerData {
    pot1: number;
    pot2: number;
    pot3: number;
}

export interface ConnectionStatus {
    connected: boolean;
    port: string | null;
    error: string | null;
}

export interface AudioSession {
    process_id: number;
    process_name: string;
    display_name: string;
    volume: number;
    is_muted: boolean;
}

export interface MixerChannel {
    id: number;
    value: number;
    is_physical: boolean;
    mapped_app: string | null;
    app_process_id: number | null;
}

export interface ChannelMapping {
    channel_id: number;
    process_id: number | null;
    process_name: string | null;
    is_master: boolean;
}

export interface SerialPortInfo {
    port_name: string;
    description: string;
}

// Stores
export const potentiometerData = writable<PotentiometerData>({
    pot1: 0,
    pot2: 0,
    pot3: 0,
});

export const connectionStatus = writable<ConnectionStatus>({
    connected: false,
    port: null,
    error: null,
});

export const audioSessions = writable<AudioSession[]>([]);
export const mixerChannels = writable<MixerChannel[]>([]);
export const availablePorts = writable<SerialPortInfo[]>([]);
export const channelMappings = writable<ChannelMapping[]>([]);

// Derived stores
export const channelValues = derived(
    [potentiometerData, mixerChannels],
    ([$potData, $channels]) => {
        return $channels.map((channel) => {
            if (channel.is_physical) {
                // Get actual pot value
                const potKey = `pot${channel.id}` as keyof PotentiometerData;
                const rawValue = $potData[potKey] || 0;
                return {
                    ...channel,
                    value: (rawValue / 4095) * 100, // Convert to percentage
                };
            }
            return channel;
        });
    }
);

// Initialize event listeners
export async function initializeListeners() {
    // Listen for potentiometer data
    await listen<PotentiometerData>('pot-data', (event: Event<PotentiometerData>) => {
        potentiometerData.set(event.payload);
    });

    // Listen for connection status changes
    await listen<ConnectionStatus>('connection-status', (event: Event<ConnectionStatus>) => {
        connectionStatus.set(event.payload);
    });

    // Listen for audio session changes
    await listen<AudioSession[]>('audio-sessions', (event: Event<AudioSession[]>) => {
        audioSessions.set(event.payload);
    });
}

// API Functions
export async function listSerialPorts(): Promise<SerialPortInfo[]> {
    try {
        const ports = await invoke<SerialPortInfo[]>('list_serial_ports');
        availablePorts.set(ports);
        return ports;
    } catch (error) {
        console.error('Failed to list serial ports:', error);
        return [];
    }
}

export async function connectSerial(port?: string): Promise<ConnectionStatus> {
    try {
        const status = await invoke<ConnectionStatus>('connect_serial', { port });
        connectionStatus.set(status);
        return status;
    } catch (error) {
        console.error('Failed to connect serial:', error);
        const status = {
            connected: false,
            port: null,
            error: error as string,
        };
        connectionStatus.set(status);
        return status;
    }
}

export async function disconnectSerial(): Promise<void> {
    try {
        await invoke('disconnect_serial');
        connectionStatus.set({
            connected: false,
            port: null,
            error: null,
        });
    } catch (error) {
        console.error('Failed to disconnect serial:', error);
    }
}

export async function getAudioSessions(): Promise<AudioSession[]> {
    try {
        const sessions = await invoke<AudioSession[]>('get_audio_sessions');
        audioSessions.set(sessions);
        return sessions;
    } catch (error) {
        console.error('Failed to get audio sessions:', error);
        return [];
    }
}

export async function setAppVolume(processId: number, volume: number): Promise<void> {
    try {
        await invoke('set_app_volume', { processId, volume });
    } catch (error) {
        console.error('Failed to set app volume:', error);
    }
}

export async function setMasterVolume(volume: number): Promise<void> {
    try {
        await invoke('set_master_volume', { volume });
    } catch (error) {
        console.error('Failed to set master volume:', error);
    }
}

export async function saveChannelMapping(mapping: ChannelMapping): Promise<void> {
    try {
        await invoke('save_channel_mapping', { mapping });
        await loadChannelMappings();
        await loadMixerChannels();
    } catch (error) {
        console.error('Failed to save channel mapping:', error);
    }
}

export async function clearChannelMapping(channelId: number): Promise<void> {
    try {
        await invoke('clear_channel_mapping', { channelId });
        await loadChannelMappings();
        await loadMixerChannels();
    } catch (error) {
        console.error('Failed to clear channel mapping:', error);
    }
}

export async function loadChannelMappings(): Promise<ChannelMapping[]> {
    try {
        const mappings = await invoke<ChannelMapping[]>('get_channel_mappings');
        channelMappings.set(mappings);
        return mappings;
    } catch (error) {
        console.error('Failed to load channel mappings:', error);
        return [];
    }
}

export async function loadMixerChannels(): Promise<MixerChannel[]> {
    try {
        const channels = await invoke<MixerChannel[]>('get_mixer_channels');
        mixerChannels.set(channels);
        return channels;
    } catch (error) {
        console.error('Failed to load mixer channels:', error);
        return [];
    }
}

// Initialize the mixer on app start
export async function initializeMixer() {
    await initializeListeners();
    await loadMixerChannels();
    await loadChannelMappings();
    await getAudioSessions();
    await listSerialPorts();

    // Try auto-connect
    await connectSerial();

    // Refresh audio sessions periodically
    setInterval(async () => {
        await getAudioSessions();
    }, 5000);
}