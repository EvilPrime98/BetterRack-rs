import { create } from 'zustand';
import { getDownloaded } from '../services/downloaded.service';

interface IDownloadedStore {
    ids: Set<number>;
    paths: Map<number, string>;
    load: () => Promise<void>;
}

export const useDownloadedStore = create<IDownloadedStore>((set) => ({
    ids: new Set(),
    paths: new Map(),

    load: async () => {
        const { ids, paths } = await getDownloaded();
        const byId = new Map(Object.entries(paths).map(([id, path]) => [Number(id), path]));
        set({ ids: new Set([...ids, ...byId.keys()]), paths: byId });
    },
}));
