import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import type { IComicLSCache } from '../library.types';
import { getComicData, updateComicData } from '../services/comic-data.service';

const PERSIST_DEBOUNCE_MS = 400;
const persistTimers = new Map<string, ReturnType<typeof setTimeout>>();
let initPromise: Promise<void> | null = null;

export interface IComicCacheCtx {
    cache: IUltraCompStateStateful<Record<string, IComicLSCache>>;
    init: () => Promise<void>;
    ready: () => Promise<void>;
    getCacheById: (uid: string) => IComicLSCache | undefined;
    setCacheById: (uid: string, pref: Partial<IComicLSCache>) => void;
    flushPending: () => Promise<void>;
    subscribeById: (uid: string, fn: (entry: IComicLSCache | undefined) => void) => () => void;
}

export const COMIC_CACHE_CONTEXT: IComicCacheCtx = ultraCompState({
    cache: {} as Record<string, IComicLSCache>,

    init: (comp: IComicCacheCtx) => {
        initPromise = (async () => {
            const data = await getComicData();
            comp.cache.set(data);
        })();
        return initPromise;
    },

    ready: () => initPromise ?? Promise.resolve(),

    getCacheById: (comp: IComicCacheCtx, uid: string) => {
        return (comp.cache.get() as Record<string, IComicLSCache>)[uid];
    },

    setCacheById: (comp: IComicCacheCtx, uid: string, pref: Partial<IComicLSCache>) => {
        const currCache = { ...comp.cache.get() };
        currCache[uid] = { ...currCache[uid], ...pref };
        comp.cache.set(currCache);

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

    flushPending: async (comp: IComicCacheCtx) => {
        const pendingUids = [...persistTimers.keys()];
        for (const timer of persistTimers.values()) clearTimeout(timer);
        persistTimers.clear();
        const cache = comp.cache.get();
        await Promise.all(
            pendingUids.map((uid) => updateComicData(uid, cache[uid]).catch(console.error)),
        );
    },

    subscribeById: (
        comp: IComicCacheCtx,
        uid: string,
        fn: (entry: IComicLSCache | undefined) => void,
    ) => {
        let lastEntry = comp.cache.get()[uid];
        return comp.cache.subscribe((cache) => {
            const entry = cache[uid];
            if (entry === lastEntry) return;
            lastEntry = entry;
            fn(entry);
        });
    },
});
