import { writable, derived } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen, type Event } from '@tauri-apps/api/event';

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

export const mixerChannels = writable<MixerChannel[]>([]);
export const availablePorts = writable<SerialPortInfo[]>([]);

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
                    value: Math.round((rawValue / 4095) * 100), // Convert to percentage and round to whole number
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

export async function setMasterVolume(volume: number): Promise<void> {
    try {
        await invoke('set_master_volume', { volume });
    } catch (error) {
        console.error('Failed to set master volume:', error);
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
    await listSerialPorts();

    // Try auto-connect
    await connectSerial();
}