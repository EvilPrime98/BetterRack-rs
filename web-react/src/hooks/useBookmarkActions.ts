import { useCallback } from 'react';
import { useBookmarksStore } from '@/stores/bookmarks.store';
import { useLibraryStore } from '@/stores/library.store';
import { useComicCacheStore } from '@/stores/comicCache.store';
import { useMoveFileModalContext } from '@/context/MoveFileModalContext.hooks';
import { useDownloadDirModalContext } from '@/context/DownloadDirModalContext.hooks';
import { useLinkPickerModalContext } from '@/context/LinkPickerModalContext.hooks';
import { getComicLinks, getDownloadJobs, startDownloadJob } from '@/services/store.service';
import { DOWNLOAD_TRACKER } from '@/services/download-guard.service';
import { toast } from '@/services/toast.service';
import { STRAT, type IStoreBookmark, type IStorePost } from '@/store.types';
import { comicUids, folderDir, onlyNewComic, postKey } from '@/lib/bookmarks';

const POLL_MS = 1000;
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

async function waitForJob(jobId: string): Promise<boolean> {
    for (;;) {
        await sleep(POLL_MS);
        try {
            const job = (await getDownloadJobs()).find((j) => j.jobId === jobId);
            if (!job || job.state === 'error') return false;
            if (job.state === 'done') return true;
        } catch {
            /* keep polling */
        }
    }
}

export function useBookmarkActions() {
    const { pickFolder } = useMoveFileModalContext();
    const { openDownloadDirModal } = useDownloadDirModalContext();
    const { openLinkPickerModal } = useLinkPickerModalContext();

    const toggle = useCallback(
        async (post: IStorePost) => {
            const store = useBookmarksStore.getState();
            const key = postKey(post);
            if (store.contains(key)) {
                await store.remove(key);
                return;
            }
            try {
                await useLibraryStore.getState().fetchLibrary();
            } catch {
                /* the picker still works with what is loaded */
            }
            const choice = await pickFolder();
            if (!choice) return;
            await store.save(post, choice.folder);
        },
        [pickFolder],
    );

    const move = useCallback(
        async (key: string) => {
            const choice = await pickFolder();
            if (!choice) return;
            await useBookmarksStore
                .getState()
                .patch(key, { folder: choice.folder ?? null }, 'Failed to move bookmark.');
        },
        [pickFolder],
    );

    const download = useCallback(
        async (bookmark: IStoreBookmark) => {
            const store = useBookmarksStore.getState();
            const { key } = bookmark;
            if (store.downloading.includes(key)) return;
            if (!bookmark.id) {
                toast.error('This post cannot be downloaded.');
                return;
            }
            const id = bookmark.id;
            store.setBusy(key, true);
            try {
                const library = useLibraryStore.getState();
                const before = comicUids(library.groups);
                const links = await getComicLinks(id, STRAT);
                if (links.length === 0) throw new Error('No download links found.');
                const link =
                    links.length === 1
                        ? links[0]
                        : await openLinkPickerModal(links, bookmark.title);
                if (!link) return;
                const outputDir =
                    folderDir(library.groups, bookmark.folder ?? undefined) ??
                    (await openDownloadDirModal(link.title));
                if (!outputDir) return;

                const { jobId } = await startDownloadJob({
                    id,
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

                await library.refreshLibrary({ silent: true });
                const uid = onlyNewComic(useLibraryStore.getState().groups, before);
                if (uid) {
                    const cache = useComicCacheStore.getState();
                    if (bookmark.rating > 0) cache.setCacheById(uid, { rating: bookmark.rating });
                    if (bookmark.read) cache.setCacheById(uid, { read: true, readPer: 100 });
                }
                await useBookmarksStore.getState().remove(key);
            } catch (e) {
                toast.error(e instanceof Error ? e.message : 'Download failed.');
            } finally {
                useBookmarksStore.getState().setBusy(key, false);
            }
        },
        [openDownloadDirModal, openLinkPickerModal],
    );

    return { toggle, move, download };
}
