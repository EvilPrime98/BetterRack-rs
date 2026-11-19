import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import { addPullEntry, deletePullEntry, getPullList } from '../services/pull-list.service';
import { toast } from '../services/toast.service';
import type { IPullEntry } from '../store.types';
import { keywordFromTitle, matches } from '../lib/pull-list';

export interface IPullListCtx {
    items: IUltraCompStateStateful<IPullEntry[]>;
    load: () => Promise<void>;
    follows: (title: string) => boolean;
    toggle: (title: string) => Promise<void>;
}

function fail(e: unknown, fallback: string) {
    toast.error(e instanceof Error ? e.message : fallback);
}

const pending = new Set<string>();

export const PULL_LIST_CTX: IPullListCtx = ultraCompState({
    items: [] as IPullEntry[],

    load: async (comp: IPullListCtx) => {
        try {
            comp.items.set(await getPullList());
        } catch (e) {
            fail(e, 'Failed to load the pull list.');
        }
    },

    follows: (comp: IPullListCtx, title: string) => {
        return comp.items.get().some((e) => matches(e, title));
    },

    toggle: async (comp: IPullListCtx, title: string) => {
        const keyword = keywordFromTitle(title);
        if (!keyword) {
            toast.error('Could not tell which series this is.');
            return;
        }
        const lock = keyword.toLowerCase();
        if (pending.has(lock)) return;
        pending.add(lock);
        const existing = comp.items.get().filter((e) => matches(e, title));
        try {
            if (existing.length > 0) {
                await Promise.all(existing.map((e) => deletePullEntry(e.id)));
                const removed = new Set(existing.map((e) => e.id));
                comp.items.set(comp.items.get().filter((e) => !removed.has(e.id)));
                toast.success(`Removed "${keyword}" from your pull list.`);
            } else {
                const saved = await addPullEntry(keyword);
                const list = comp.items.get();
                if (!list.some((e) => e.id === saved.id)) comp.items.set([...list, saved]);
                toast.success(`Added "${keyword}" to your pull list.`);
            }
        } catch (e) {
            fail(e, 'Failed to update the pull list.');
        } finally {
            pending.delete(lock);
        }
    },
});
