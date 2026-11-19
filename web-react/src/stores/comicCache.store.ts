import { create } from 'zustand';
import type { IComicLSCache } from '../library.types';
import { getComicData, updateComicData } from '../services/comic-data.service';

const PERSIST_DEBOUNCE_MS = 400;
const persistTimers = new Map<string, ReturnType<typeof setTimeout>>();
let initPromise: Promise<void> | null = null;

interface IComicCacheStore {
    cache: Record<string, IComicLSCache>;
    init: () => Promise<void>;
    ready: () => Promise<void>;
    getCacheById: (uid: string) => IComicLSCache | undefined;
    setCacheById: (uid: string, pref: Partial<IComicLSCache>) => void;
    flushPending: () => Promise<void>;
    subscribeById: (uid: string, fn: (entry: IComicLSCache | undefined) => void) => () => void;
}

export const useComicCacheStore = create<IComicCacheStore>((set, get, api) => ({
    cache: {},

    init: () => {
        initPromise = (async () => {
            const data = await getComicData();
            set({ cache: data });
        })();
        return initPromise;
    },

    ready: () => initPromise ?? Promise.resolve(),

    getCacheById: (uid) => get().cache[uid],

    setCacheById: (uid, pref) => {
        const currCache = { ...get().cache };
        currCache[uid] = { ...currCache[uid], ...pref };
        set({ cache: currCache });

        const existingTimer = persistTimers.get(uid);
        if (existingTimer) clearTimeout(existingTimer);
        persistTimers.set(
            uid,
            setTimeout(() => {
                persistTimers.delete(uid);
                updateComicData(uid, currCache[uid]).catch(console.error);
            }, PERSIST_DEBOUNCE_MS),
        );
    },

    flushPending: async () => {
        const pendingUids = [...persistTimers.keys()];
        for (const timer of persistTimers.values()) clearTimeout(timer);
        persistTimers.clear();
        await Promise.all(
            pendingUids.map((uid) => updateComicData(uid, get().cache[uid]).catch(console.error)),
        );
    },

    subscribeById: (uid, fn) => {
        let lastEntry = get().cache[uid];
        return api.subscribe((state) => {
            const entry = state.cache[uid];
            if (entry === lastEntry) return;
            lastEntry = entry;
            fn(entry);
        });
    },
}));
