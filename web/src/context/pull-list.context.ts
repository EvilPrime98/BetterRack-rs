import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import { addPullEntry, deletePullEntry, getPullList } from '../services/pull-list.service';
import { toast } from '../services/toast.service';
import type { IPullEntry } from '../store.types';
import { keywordFromTitle } from '../lib/pull-list';

export interface IPullListCtx {
    items: IUltraCompStateStateful<IPullEntry[]>;
    load: () => Promise<void>;
    follows: (title: string) => boolean;
    toggle: (title: string) => Promise<void>;
}

function fail(e: unknown, fallback: string) {
    toast.error(e instanceof Error ? e.message : fallback);
}

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

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
        const keyword = keywordFromTitle(title);
        return comp.items.get().some((e) => same(e.keyword, keyword));
    },

    /** The pin on a store card: follow the series, or unfollow it when already followed. */
    toggle: async (comp: IPullListCtx, title: string) => {
        const keyword = keywordFromTitle(title);
        if (!keyword) {
            toast.error('Could not tell which series this is.');
            return;
        }
        const existing = comp.items.get().find((e) => same(e.keyword, keyword));
        try {
            if (existing) {
                await deletePullEntry(existing.id);
                comp.items.set(comp.items.get().filter((e) => e.id !== existing.id));
                toast.success(`Removed "${keyword}" from your pull list.`);
            } else {
                const saved = await addPullEntry(keyword, '');
                comp.items.set([...comp.items.get(), saved]);
                toast.success(`Added "${keyword}" to your pull list.`);
            }
        } catch (e) {
            fail(e, 'Failed to update the pull list.');
        }
    },
});
