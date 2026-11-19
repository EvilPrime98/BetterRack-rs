import { create } from 'zustand';
import {
    getStoredServerUrl,
    isRemoteModeEnabled,
    needsServerSetup,
    setServerUrl,
    setRemoteServer,
    setStoredApiKey,
    isAndroidPlatform,
} from '../services/server-config.service';

export { getStoredServerUrl };

let resolver: (() => void) | null = null;

interface IServerModalStore {
    isVisible: boolean;
    isMandatory: boolean;
    error: string;
    /** Resolves immediately if no server setup is needed, otherwise opens the modal and waits for submit. */
    ensureServerConfigured: () => Promise<void>;
    openServerModal: () => void;
    closeServerModal: () => void;
    submitServer: (url: string, remote?: { enabled: boolean; apiKey: string }) => void;
}

export const useServerModalStore = create<IServerModalStore>((set, get) => ({
    isVisible: false,
    isMandatory: false,
    error: '',

    ensureServerConfigured: (): Promise<void> => {
        if (!needsServerSetup()) return Promise.resolve();
        set({ error: '', isMandatory: true, isVisible: true });
        return new Promise<void>((resolve) => {
            resolver = resolve;
        });
    },

    openServerModal: () => {
        set({ error: '', isMandatory: false, isVisible: true });
    },

    closeServerModal: () => {
        if (get().isMandatory) return;
        set({ isVisible: false });
    },

    submitServer: (url, remote) => {
        const trimmed = url.trim();
        if (!trimmed) {
            set({ error: 'Enter a server address.' });
            return;
        }
        const urlBefore = getStoredServerUrl();
        const remoteBefore = isRemoteModeEnabled();
        if (remote?.enabled) setRemoteServer(trimmed, remote.apiKey.trim());
        else {
            setServerUrl(trimmed);
            if (isAndroidPlatform()) setStoredApiKey(remote?.apiKey.trim() ?? '');
        }
        set({ isVisible: false, isMandatory: false });
        resolver?.();
        resolver = null;
        if (getStoredServerUrl() !== urlBefore || isRemoteModeEnabled() !== remoteBefore) {
            window.location.reload();
        }
    },
}));
