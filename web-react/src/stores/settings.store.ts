import { create } from 'zustand';
import {
    getSettings,
    updateSettings as requestUpdateSettings,
    addLibraryFolder as requestAddLibraryFolder,
    removeLibraryFolder as requestRemoveLibraryFolder,
} from '../services/settings.service';
import type { IAppSettings } from '../settings.types';

interface ISettingsStore {
    settings: IAppSettings;
    fetchSettings: () => Promise<void>;
    updateSettings: (partial: Partial<Omit<IAppSettings, 'outputDirs'>>) => Promise<void>;
    addLibraryFolder: (path: string) => Promise<void>;
    removeLibraryFolder: (path: string) => Promise<void>;
}

export const useSettingsStore = create<ISettingsStore>((set) => ({
    settings: {
        outputDirs: [],
        apiUrl: '',
        downloadDir: '',
        wikiSearch: false,
        rescanOnStartup: true,
        rotatingFetch: false,
    },

    fetchSettings: async () => {
        const data = await getSettings();
        set({ settings: data });
    },

    updateSettings: async (partial) => {
        const data = await requestUpdateSettings(partial);
        set({ settings: data });
    },

    addLibraryFolder: async (path) => {
        const data = await requestAddLibraryFolder(path);
        set({ settings: data });
    },

    removeLibraryFolder: async (path) => {
        const data = await requestRemoveLibraryFolder(path);
        set({ settings: data });
    },
}));
