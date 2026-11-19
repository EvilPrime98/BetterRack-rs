import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import {
    addBookmark,
    deleteBookmark,
    getBookmarks,
    patchBookmark,
} from '../services/bookmarks.service';
import { getComicLinks, getDownloadJobs, startDownloadJob } from '../services/store.service';
import { DOWNLOAD_TRACKER } from '../services/download-guard.service';
import { toast } from '../services/toast.service';
import { STRAT, type IStoreBookmark, type IStoreLink, type IStorePost } from '../store.types';
import { comicUids, folderDir, onlyNewComic, postKey } from '../lib/bookmarks';
import { LIBRARY_CONTEXT } from './library.context';
import { COMIC_CACHE_CONTEXT } from './comic-cache.context';
import { DOWNLOAD_DIR_MODAL_CTX } from './download-dir-modal.context';
import { LINK_PICKER_MODAL_CTX } from './link-picker-modal.context';
import { MOVE_FILE_MODAL_CTX } from './move-file-modal.context';

const POLL_MS = 1000;
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export interface IBookmarksCtx {
    items: IUltraCompStateStateful<IStoreBookmark[]>;
    downloading: IUltraCompStateStateful<string[]>;
    load: () => Promise<void>;
    contains: (key: string) => boolean;
    toggle: (post: IStorePost) => Promise<void>;
    move: (key: string) => Promise<void>;
    setRating: (key: string, rating: number) => Promise<void>;
    setRead: (key: string, read: boolean) => Promise<void>;
    remove: (key: string) => Promise<void>;
    download: (key: string) => Promise<void>;
}

function fail(e: unknown, fallback: string) {
    toast.error(e instanceof Error ? e.message : fallback);
}

function replace(comp: IBookmarksCtx, saved: IStoreBookmark) {
    const list = comp.items.get();
    comp.items.set(
        list.some((b) => b.key === saved.key)
            ? list.map((b) => (b.key === saved.key ? saved : b))
            : [...list, saved],
    );
}

async function chooseLink(bookmark: IStoreBookmark): Promise<IStoreLink | null> {
    const links = await getComicLinks(bookmark.id as number, STRAT);
    if (links.length === 0) throw new Error('No download links found.');
    if (links.length === 1) return links[0];
    return LINK_PICKER_MODAL_CTX.openLinkPickerModal(links, bookmark.title);
}

async function waitForJob(jobId: string): Promise<boolean> {
    for (;;) {
        await sleep(POLL_MS);
        try {
            const job = (await getDownloadJobs()).find((j) => j.jobId === jobId);
            if (!job || job.state === 'error') return false;
            if (job.state === 'done') return true;
        } catch {
        }
    }
}

export const BOOKMARKS_CTX: IBookmarksCtx = ultraCompState({
    items: [] as IStoreBookmark[],
    downloading: [] as string[],

    load: async (comp: IBookmarksCtx) => {
        try {
            comp.items.set(await getBookmarks());
        } catch (e) {
            fail(e, 'Failed to load bookmarks.');
        }
    },

    contains: (comp: IBookmarksCtx, key: string) => comp.items.get().some((b) => b.key === key),

    toggle: async (comp: IBookmarksCtx, post: IStorePost) => {
        const key = postKey(post);
        try {
            if (comp.contains(key)) {
                await deleteBookmark(key);
                comp.items.set(comp.items.get().filter((b) => b.key !== key));
                return;
            }
            await LIBRARY_CONTEXT.fetchLibrary();
            const choice = await MOVE_FILE_MODAL_CTX.pickFolder();
            if (!choice) return;
            replace(comp, await addBookmark(post, choice.folder));
        } catch (e) {
            fail(e, 'Failed to update bookmark.');
        }
    },

    move: async (comp: IBookmarksCtx, key: string) => {
        const choice = await MOVE_FILE_MODAL_CTX.pickFolder();
        if (!choice) return;
        try {
            replace(comp, await patchBookmark(key, { folder: choice.folder ?? null }));
        } catch (e) {
            fail(e, 'Failed to move bookmark.');
        }
    },

    setRating: async (comp: IBookmarksCtx, key: string, rating: number) => {
        try {
            replace(comp, await patchBookmark(key, { rating }));
        } catch (e) {
            fail(e, 'Failed to rate bookmark.');
        }
    },

    setRead: async (comp: IBookmarksCtx, key: string, read: boolean) => {
        try {
            replace(comp, await patchBookmark(key, { read }));
        } catch (e) {
            fail(e, 'Failed to update bookmark.');
        }
    },

    remove: async (comp: IBookmarksCtx, key: string) => {
        try {
            await deleteBookmark(key);
            comp.items.set(comp.items.get().filter((b) => b.key !== key));
        } catch (e) {
            fail(e, 'Failed to remove bookmark.');
        }
    },

    download: async (comp: IBookmarksCtx, key: string) => {
        const bookmark = comp.items.get().find((b) => b.key === key);
        if (!bookmark || comp.downloading.get().includes(key)) return;
        if (!bookmark.id) {
            toast.error('This post cannot be downloaded.');
            return;
        }
        const setBusy = (busy: boolean) =>
            comp.downloading.set(
                busy
                    ? [...comp.downloading.get(), key]
                    : comp.downloading.get().filter((k) => k !== key),
            );

        setBusy(true);
        try {
            const groups = LIBRARY_CONTEXT.groups.get();
            const before = comicUids(groups);
            const link = await chooseLink(bookmark);
            if (!link) return;
            const outputDir =
                folderDir(groups, bookmark.folder ?? undefined) ??
                (await DOWNLOAD_DIR_MODAL_CTX.openDownloadDirModal(link.title));
            if (!outputDir) return;

            const { jobId } = await startDownloadJob({
                id: bookmark.id,
                title: link.title,
                uuid: link.uuid,
                outputDir,
                strat: STRAT,
            });
            DOWNLOAD_TRACKER.started();
            toast.success('Download in progress');

            if (!(await waitForJob(jobId))) {
                toast.error('Download failed.');
                return;
            }

            await LIBRARY_CONTEXT.refreshLibrary({ silent: true });
            const uid = onlyNewComic(LIBRARY_CONTEXT.groups.get(), before);
            if (uid) {
                if (bookmark.rating > 0) {
                    COMIC_CACHE_CONTEXT.setCacheById(uid, { rating: bookmark.rating });
                }
                if (bookmark.read) {
                    COMIC_CACHE_CONTEXT.setCacheById(uid, { read: true, readPer: 100 });
                }
            }
            await comp.remove(key);
        } catch (e) {
            fail(e, 'Download failed.');
        } finally {
            setBusy(false);
        }
    },
});
