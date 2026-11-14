import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import {
    addPullEntry,
    deletePullEntry,
    getPullList,
    patchPullEntry,
} from '../services/pull-list.service';
import { toast } from '../services/toast.service';
import type { IPullEntry } from '../store.types';
import { keywordFromTitle, latestMatchId } from '../lib/pull-list';
import { STORE_PAGE_CONTEXT } from './store-page.context';

export interface IPullListCtx {
    items: IUltraCompStateStateful<IPullEntry[]>;
    focus: IUltraCompStateStateful<string>;
    prefill: IUltraCompStateStateful<string>;
    load: () => Promise<void>;
    requestAdd: (title: string) => void;
    add: (keyword: string, exclude: string) => Promise<boolean>;
    remove: (id: string) => Promise<void>;
    markSeen: (id: string) => Promise<void>;
}

function fail(e: unknown, fallback: string) {
    toast.error(e instanceof Error ? e.message : fallback);
}

export const PULL_LIST_CTX: IPullListCtx = ultraCompState({
    items: [] as IPullEntry[],
    focus: '',
    prefill: '',

    load: async (comp: IPullListCtx) => {
        try {
            comp.items.set(await getPullList());
        } catch (e) {
            fail(e, 'Failed to load the pull list.');
        }
    },

    requestAdd: (comp: IPullListCtx, title: string) => {
        comp.prefill.set(keywordFromTitle(title));
    },

    add: async (comp: IPullListCtx, keyword: string, exclude: string) => {
        if (!keyword.trim()) return false;
        try {
            const saved = await addPullEntry(keyword, exclude);
            const list = comp.items.get();
            if (!list.some((e) => e.id === saved.id)) comp.items.set([...list, saved]);
            return true;
        } catch (e) {
            fail(e, 'Failed to add to the pull list.');
            return false;
        }
    },

    remove: async (comp: IPullListCtx, id: string) => {
        try {
            await deletePullEntry(id);
            if (comp.focus.get() === id) comp.focus.set('');
            comp.items.set(comp.items.get().filter((e) => e.id !== id));
        } catch (e) {
            fail(e, 'Failed to remove from the pull list.');
        }
    },

    markSeen: async (comp: IPullListCtx, id: string) => {
        const entry = comp.items.get().find((e) => e.id === id);
        if (!entry) return;
        try {
            const saved = await patchPullEntry(id, {
                lastSeenId: latestMatchId(entry, STORE_PAGE_CONTEXT.results.get()),
            });
            comp.items.set(comp.items.get().map((e) => (e.id === id ? saved : e)));
        } catch (e) {
            fail(e, 'Failed to mark as seen.');
        }
    },
});
