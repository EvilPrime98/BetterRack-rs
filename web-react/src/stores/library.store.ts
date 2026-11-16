import { create } from 'zustand';
import {
    getLibrary,
    refreshLibrary as requestLibraryRefresh,
    deleteFile as requestDeleteFile,
    deleteFolder as requestDeleteFolder,
    createFolder as requestCreateFolder,
    flattenFolder as requestFlattenFolder,
    moveFile as requestMoveFile,
    unidentifyFile as requestUnidentifyFile,
    reidentifyAllLibrary as requestReidentifyAllLibrary,
    startIdentifyLibrary,
    getIdentifyLibraryStatus,
    type TLibraryStructure,
} from '../services/library.service';
import type {
    ILibraryGroup,
    ILibraryResponseItem,
    TIdentifyLibraryStatus,
    TIdentifyProgress,
} from '../library.types';
import { toast } from '../services/toast.service';
import { useConfirmModalStore } from './confirmModal.store';

// React Query's hooks only work inside components, so rather than force a useQuery call
// into this store action, fetchLibrary keeps a lightweight staleTime + in-flight-promise
// cache here instead.
const LIBRARY_STALE_TIME_MS = 60 * 5 * 1000;
const cache: Partial<Record<TLibraryStructure, { groups: ILibraryGroup[]; fetchedAt: number }>> =
    {};
const inFlight: Partial<Record<TLibraryStructure, Promise<void>>> = {};

function invalidateLibraryCache() {
    delete cache.folders;
    delete cache.series;
}

const IDENTIFY_POLL_INTERVAL_MS = 1000;
let identifyPolling = false;

const STRUCTURE_STORAGE_KEY = 'library-structure';

function readStoredStructure(): TLibraryStructure {
    try {
        return localStorage.getItem(STRUCTURE_STORAGE_KEY) === 'series' ? 'series' : 'folders';
    } catch {
        return 'folders';
    }
}

interface ILibraryStore {
    groups: ILibraryGroup[];
    searchQuery: string;
    structure: TLibraryStructure;
    /**
     * Holds the uid of the last deleted entry. The /new view (through useRecentlyAdded)
     * reads this to remove that entry from its own list snapshot without an app reload.
     * Each deletion sets a new object, so repeated deletes of the same uid stay distinct
     * and the consuming effect runs again.
     */
    lastDeleted: { uid: string } | null;
    identifyProgress: Extract<TIdentifyProgress, { type: 'identifying' }> | null;
    setSearchQuery: (query: string) => void;
    setStructure: (structure: TLibraryStructure) => Promise<void>;
    fetchLibrary: () => Promise<void>;
    refreshLibrary: (options?: { silent?: boolean }) => Promise<void>;
    refreshLibraryWithPrompt: () => Promise<void>;
    identifyLibrary: () => Promise<void>;
    deleteFile: (uid: string) => Promise<void>;
    deleteFolder: (uid: string) => Promise<void>;
    createFolder: (folderName: string, parentFolderUid?: string) => Promise<void>;
    moveFile: (fileUid: string, targetFolderUid?: string) => Promise<void>;
    flattenFolder: (folderUid?: string) => Promise<void>;
    unidentifyFile: (uid: string) => Promise<void>;
    reidentifyAll: () => Promise<void>;
    getLibraryItems: (args: { onlyDir: boolean; uid?: string }) => ILibraryResponseItem[];
    findUidByPath: (absPath: string) => string | undefined;
}

export const useLibraryStore = create<ILibraryStore>((set, get) => ({
    groups: [],
    searchQuery: '',
    structure: readStoredStructure(),
    lastDeleted: null,
    identifyProgress: null,

    setSearchQuery: (searchQuery) => set({ searchQuery }),

    setStructure: async (structure) => {
        if (get().structure === structure) return;
        try {
            localStorage.setItem(STRUCTURE_STORAGE_KEY, structure);
        } catch {
            /* storage unavailable */
        }
        const cached = cache[structure];
        set(cached ? { structure, groups: cached.groups } : { structure });
        try {
            await get().fetchLibrary();
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to load library.');
        }
    },

    fetchLibrary: async () => {
        const structure = get().structure;
        const cached = cache[structure];
        if (
            cached &&
            Date.now() - cached.fetchedAt < LIBRARY_STALE_TIME_MS &&
            !inFlight[structure]
        ) {
            if (get().groups !== cached.groups) set({ groups: cached.groups });
            return;
        }
        if (!inFlight[structure]) {
            inFlight[structure] = (async () => {
                const data = await getLibrary(structure);
                cache[structure] = { groups: data, fetchedAt: Date.now() };
                if (get().structure === structure) set({ groups: data });
            })().finally(() => {
                delete inFlight[structure];
            });
        }
        await inFlight[structure];
    },

    refreshLibrary: async (options) => {
        try {
            await requestLibraryRefresh();
            invalidateLibraryCache();
            await get().fetchLibrary();
            if (!options?.silent) toast.success('Library refreshed');
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to refresh library.');
        }
    },

    refreshLibraryWithPrompt: async () => {
        await get().refreshLibrary({ silent: true });

        if (get().groups.every((g) => g.entries.length === 0)) {
            toast.success('Library refreshed');
            return;
        }

        const identify = await useConfirmModalStore.getState().confirmDialog({
            title: 'Identify library?',
            message:
                'Do you also want to identify every comic in your library after refreshing? This can take a while and runs in the background.',
            confirmLabel: 'Refresh & identify',
            cancelLabel: 'Just refresh',
        });

        if (identify === null) return;

        if (!identify) {
            toast.success('Library refreshed');
            return;
        }

        await get().identifyLibrary();
    },

    identifyLibrary: async () => {
        if (identifyPolling) return;

        identifyPolling = true;
        try {
            let status: TIdentifyLibraryStatus = await startIdentifyLibrary();

            for (;;) {
                if (status.state === 'idle') return;
                const progress = status.progress;

                if (status.state === 'error' || progress?.type === 'error') {
                    toast.error(
                        progress?.type === 'error'
                            ? progress.message
                            : 'Library identification failed.',
                    );
                    return;
                }
                if (status.state === 'done' || progress?.type === 'done') {
                    invalidateLibraryCache();
                    await get().fetchLibrary();
                    toast.success('Library identified');
                    return;
                }

                set({
                    identifyProgress:
                        progress?.type === 'identifying'
                            ? progress
                            : { type: 'identifying', done: 0, total: 0 },
                });
                await new Promise((resolve) => setTimeout(resolve, IDENTIFY_POLL_INTERVAL_MS));
                status = await getIdentifyLibraryStatus();
            }
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to identify library.');
        } finally {
            identifyPolling = false;
            set({ identifyProgress: null });
        }
    },

    deleteFile: async (uid) => {
        try {
            const data = await requestDeleteFile(uid);
            invalidateLibraryCache();
            await get().fetchLibrary();
            set({ lastDeleted: { uid } });
            toast.success(data.message || 'File deleted');
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to delete file.');
        }
    },

    deleteFolder: async (uid) => {
        try {
            const data = await requestDeleteFolder(uid);
            invalidateLibraryCache();
            await get().fetchLibrary();
            set({ lastDeleted: { uid } });
            toast.success(data.message || 'Folder deleted');
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to delete folder.');
        }
    },

    createFolder: async (folderName, parentFolderUid) => {
        try {
            const data = await requestCreateFolder(folderName, parentFolderUid);
            invalidateLibraryCache();
            await get().fetchLibrary();
            toast.success(data.message || 'Folder created');
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to create folder.');
        }
    },

    moveFile: async (fileUid, targetFolderUid) => {
        try {
            const data = await requestMoveFile(fileUid, targetFolderUid);
            invalidateLibraryCache();
            await get().fetchLibrary();
            toast.success(data.message || 'File moved');
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to move file.');
        }
    },

    flattenFolder: async (folderUid) => {
        try {
            const data = await requestFlattenFolder(folderUid);
            invalidateLibraryCache();
            await get().fetchLibrary();
            toast.success(data.message || 'Folder flattened');
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to flatten folder.');
        }
    },

    unidentifyFile: async (uid) => {
        try {
            const data = await requestUnidentifyFile(uid);
            invalidateLibraryCache();
            await get().fetchLibrary();
            toast.success(data.message || 'File un-identified');
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to un-identify file.');
        }
    },

    reidentifyAll: async () => {
        try {
            const data = await requestReidentifyAllLibrary();
            invalidateLibraryCache();
            await get().fetchLibrary();
            toast.success(data.message || 'Library flagged for re-identification');
        } catch (e) {
            toast.error(
                e instanceof Error ? e.message : 'Failed to flag library for re-identification.',
            );
        }
    },

    getLibraryItems: ({ onlyDir, uid }) => {
        const groups = get().groups;

        if (get().structure === 'series') {
            if (!uid) {
                return onlyDir
                    ? groups.map((g) => ({
                          uid: g.uid,
                          did: true,
                          name: g.name,
                          path: g.path,
                          parentId: '',
                          createdAt: 0,
                      }))
                    : groups.flatMap((g) => g.entries);
            }
            const series = groups.find((g) => g.uid === uid);
            if (series) return onlyDir ? [] : series.entries;
        }

        let data: ILibraryResponseItem[];

        const library = uid ? groups.find((g) => g.uid === uid) : undefined;

        if (!uid) {
            data = groups.map((g) => g.entries).flat();
        } else if (library) {
            data = library.entries.filter((e) => !e.parentId);
        } else {
            data = groups.flatMap((g) => g.entries).filter((e) => e.parentId === uid);
        }

        if (onlyDir) data = data.filter((i) => i.did !== false);

        return data;
    },

    findUidByPath: (absPath) => {
        const groups = get().groups;

        const group = groups.find((g) => g.path === absPath);
        if (group) return group.uid;

        for (const g of groups) {
            const entry = g.entries.find((e) => e.did && e.path === absPath);
            if (entry) return entry.uid;
        }

        return undefined;
    },
}));
