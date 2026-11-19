import { create } from 'zustand';
import { addPullEntry, deletePullEntry, getPullList } from '../services/pull-list.service';
import { toast } from '../services/toast.service';
import type { IPullEntry } from '../store.types';
import { keywordFromTitle, matches } from '../lib/pull-list';

interface IPullListStore {
    items: IPullEntry[];
    load: () => Promise<void>;
    toggle: (title: string) => Promise<void>;
    remove: (entry: IPullEntry) => Promise<void>;
}

function fail(e: unknown, fallback: string) {
    toast.error(e instanceof Error ? e.message : fallback);
}

const pending = new Set<string>();

export const usePullListStore = create<IPullListStore>((set, get) => ({
    items: [],

    load: async () => {
        try {
            set({ items: await getPullList() });
        } catch (e) {
            fail(e, 'Failed to load the pull list.');
        }
    },

    toggle: async (title) => {
        const keyword = keywordFromTitle(title);
        if (!keyword) {
            toast.error('Could not tell which series this is.');
            return;
        }
        const lock = keyword.toLowerCase();
        if (pending.has(lock)) return;
        pending.add(lock);
        const existing = get().items.filter((e) => matches(e, title));
        try {
            if (existing.length > 0) {
                await Promise.all(existing.map((e) => deletePullEntry(e.id)));
                const removed = new Set(existing.map((e) => e.id));
                set({ items: get().items.filter((e) => !removed.has(e.id)) });
                toast.success(`Removed "${keyword}" from your pull list.`);
            } else {
                const saved = await addPullEntry(keyword);
                const list = get().items;
                if (!list.some((e) => e.id === saved.id)) set({ items: [...list, saved] });
                toast.success(`Added "${keyword}" to your pull list.`);
            }
        } catch (e) {
            fail(e, 'Failed to update the pull list.');
        } finally {
            pending.delete(lock);
        }
    },

    remove: async (entry) => {
        if (pending.has(entry.id)) return;
        pending.add(entry.id);
        try {
            await deletePullEntry(entry.id);
            set({ items: get().items.filter((e) => e.id !== entry.id) });
        } catch (e) {
            fail(e, 'Failed to update the pull list.');
        } finally {
            pending.delete(entry.id);
        }
    },
}));
