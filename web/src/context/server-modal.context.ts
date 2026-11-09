import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import {
    getStoredServerUrl,
    isRemoteModeEnabled,
    needsServerSetup,
    setServerUrl,
    setRemoteServer,
    setStoredApiKey,
    isAndroidPlatform,
} from '../services/server-config.service';

let resolver: (() => void) | null = null;

export interface IServerModalCtx {
    isVisible: IUltraCompStateStateful<boolean>;
    isMandatory: IUltraCompStateStateful<boolean>;
    error: IUltraCompStateStateful<string>;
    ensureServerConfigured: () => Promise<void>;
    openServerModal: () => void;
    closeServerModal: () => void;
    submitServer: (url: string, remote?: { enabled: boolean; apiKey: string }) => void;
}

export const SERVER_MODAL_CTX: IServerModalCtx = ultraCompState({
    isVisible: false,
    isMandatory: false,
    error: '' as string,

    ensureServerConfigured: (comp: IServerModalCtx): Promise<void> => {
        if (!needsServerSetup()) return Promise.resolve();
        comp.error.set('');
        comp.isMandatory.set(true);
        comp.isVisible.set(true);
        return new Promise<void>((resolve) => {
            resolver = resolve;
        });
    },

    openServerModal: (comp: IServerModalCtx) => {
        comp.error.set('');
        comp.isMandatory.set(false);
        comp.isVisible.set(true);
    },

    closeServerModal: (comp: IServerModalCtx) => {
        if (comp.isMandatory.get()) return;
        comp.isVisible.set(false);
    },

    submitServer: (
        comp: IServerModalCtx,
        url: string,
        remote?: { enabled: boolean; apiKey: string },
    ) => {
        const trimmed = url.trim();
        if (!trimmed) {
            comp.error.set('Enter a server address.');
            return;
        }
        const urlBefore = getStoredServerUrl();
        const remoteBefore = isRemoteModeEnabled();
        if (remote?.enabled) setRemoteServer(trimmed, remote.apiKey.trim());
        else {
            setServerUrl(trimmed);
            if (isAndroidPlatform()) setStoredApiKey(remote?.apiKey.trim() ?? '');
        }
        comp.isVisible.set(false);
        comp.isMandatory.set(false);
        resolver?.();
        resolver = null;
        if (getStoredServerUrl() !== urlBefore || isRemoteModeEnabled() !== remoteBefore) {
            window.location.reload();
        }
    },
});

export { getStoredServerUrl };
