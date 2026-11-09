import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import {
    getSettings,
    updateSettings as requestUpdateSettings,
    addLibraryFolder as requestAddLibraryFolder,
    removeLibraryFolder as requestRemoveLibraryFolder,
} from '../services/settings.service';
import type { IAppSettings } from '../settings.types';

export interface ISettingsCtx {
    settings: IUltraCompStateStateful<IAppSettings>;
    fetchSettings: () => Promise<void>;
    updateSettings: (partial: Partial<Omit<IAppSettings, 'outputDirs'>>) => Promise<void>;
    addLibraryFolder: (path: string) => Promise<void>;
    removeLibraryFolder: (path: string) => Promise<void>;
}

export const SETTINGS_CONTEXT: ISettingsCtx = ultraCompState({
    settings: {
        outputDirs: [],
        apiUrl: '',
        downloadDir: '',
        wikiSearch: false,
        rescanOnStartup: true,
        rotatingFetch: false,
    } as IAppSettings,

    fetchSettings: async (comp: ISettingsCtx) => {
        const data = await getSettings();
        comp.settings.set(data);
    },

    updateSettings: async (
        comp: ISettingsCtx,
        partial: Partial<Omit<IAppSettings, 'outputDirs'>>,
    ) => {
        const data = await requestUpdateSettings(partial);
        comp.settings.set(data);
    },

    addLibraryFolder: async (comp: ISettingsCtx, path: string) => {
        const data = await requestAddLibraryFolder(path);
        comp.settings.set(data);
    },

    removeLibraryFolder: async (comp: ISettingsCtx, path: string) => {
        const data = await requestRemoveLibraryFolder(path);
        comp.settings.set(data);
    },
});
