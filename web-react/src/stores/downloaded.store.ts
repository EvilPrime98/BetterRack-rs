import { create } from 'zustand';
import { getDownloadedIds } from '../services/downloaded.service';

interface IDownloadedStore {
    ids: Set<number>;
    load: () => Promise<void>;
    mark: (id: number) => void;
}

export const useDownloadedStore = create<IDownloadedStore>((set, get) => ({
    ids: new Set(),

    load: async () => {
        const loaded = await getDownloadedIds();
        set({ ids: new Set([...get().ids, ...loaded]) });
    },

    mark: (id) => {
        if (!get().ids.has(id)) set({ ids: new Set(get().ids).add(id) });
    },
}));
