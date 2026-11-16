import { create } from 'zustand';
import {
    addBookmark,
    deleteBookmark,
    getBookmarks,
    patchBookmark,
} from '../services/bookmarks.service';
import { toast } from '../services/toast.service';
import type { IStoreBookmark, IStorePost } from '../store.types';

interface IBookmarkPatch {
    folder?: string | null;
    rating?: number;
    read?: boolean;
}

interface IBookmarksStore {
    items: IStoreBookmark[];
    downloading: string[];
    load: () => Promise<void>;
    contains: (key: string) => boolean;
    save: (post: IStorePost, folder?: string) => Promise<void>;
    patch: (key: string, patch: IBookmarkPatch, failure: string) => Promise<void>;
    remove: (key: string) => Promise<void>;
    setBusy: (key: string, busy: boolean) => void;
}

function fail(e: unknown, fallback: string) {
    toast.error(e instanceof Error ? e.message : fallback);
}

export const useBookmarksStore = create<IBookmarksStore>((set, get) => {
    const replace = (saved: IStoreBookmark) => {
        const list = get().items;
        set({
            items: list.some((b) => b.key === saved.key)
                ? list.map((b) => (b.key === saved.key ? saved : b))
                : [...list, saved],
        });
    };

    return {
        items: [],
        downloading: [],

        load: async () => {
            try {
                set({ items: await getBookmarks() });
            } catch (e) {
                fail(e, 'Failed to load bookmarks.');
            }
        },

        contains: (key) => get().items.some((b) => b.key === key),

        save: async (post, folder) => {
            try {
                replace(await addBookmark(post, folder));
            } catch (e) {
                fail(e, 'Failed to update bookmark.');
            }
        },

        patch: async (key, patch, failure) => {
            try {
                replace(await patchBookmark(key, patch));
            } catch (e) {
                fail(e, failure);
            }
        },

        remove: async (key) => {
            try {
                await deleteBookmark(key);
                set({ items: get().items.filter((b) => b.key !== key) });
            } catch (e) {
                fail(e, 'Failed to remove bookmark.');
            }
        },

        setBusy: (key, busy) =>
            set({
                downloading: busy
                    ? [...get().downloading, key]
                    : get().downloading.filter((k) => k !== key),
            }),
    };
});
