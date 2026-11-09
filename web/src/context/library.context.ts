import { ultraCompState, ultraQuery, type IUltraCompStateStateful } from 'ultra-light-js';
import {
    getLibraryIndex,
    getLibraryPage,
    LIBRARY_PAGE_SIZE,
    refreshLibrary as requestLibraryRefresh,
    deleteFile as requestDeleteFile,
    deleteFolder as requestDeleteFolder,
    createFolder as requestCreateFolder,
    moveFile as requestMoveFile,
    unidentifyFile as requestUnidentifyFile,
    reidentifyAllLibrary as requestReidentifyAllLibrary,
    startIdentifyLibrary,
    getIdentifyLibraryStatus,
    type TLibraryStructure,
} from '../services/library.service';
import type {
    ILibraryGroup,
    ILibraryIndexGroup,
    ILibraryPage,
    ILibraryResponseItem,
    TIdentifyLibraryStatus,
    TIdentifyProgress,
} from '../library.types';
import { toast } from '../services/toast.service';
import { CONFIRM_MODAL_CTX } from './confirm-modal.context';

const queryClient = ultraQuery();

const LIBRARY_STALE_TIME = 60 * 5 * 10000;

let libraryLoadInFlight: Promise<void> | null = null;
let libraryLoadId = 0;

const IDENTIFY_POLL_INTERVAL_MS = 1000;
let identifyPolling = false;

const STRUCTURE_STORAGE_KEY = 'library-structure';

const structureCache: Partial<Record<TLibraryStructure, ILibraryGroup[]>> = {};

function readStoredStructure(): TLibraryStructure {
    try {
        return localStorage.getItem(STRUCTURE_STORAGE_KEY) === 'series' ? 'series' : 'folders';
    } catch {
        return 'folders';
    }
}

function mergeLibraryPage(existing: ILibraryGroup[], page: ILibraryPage): ILibraryGroup[] {
    const byUid = new Map<string, ILibraryGroup>(
        existing.map((group) => [group.uid, { ...group, entries: [...group.entries] }]),
    );
    for (const incoming of page.groups) {
        const current = byUid.get(incoming.uid);
        if (!current) {
            byUid.set(incoming.uid, { ...incoming, entries: [...incoming.entries] });
            continue;
        }
        current.name = incoming.name;
        current.path = incoming.path;
        const seen = new Set(current.entries.map((entry) => entry.uid));
        for (const entry of incoming.entries) {
            if (!seen.has(entry.uid)) current.entries.push(entry);
        }
    }
    return [...byUid.values()];
}

async function reloadLibrary(comp: ILibraryCtx) {
    queryClient.invalidateCache('library');
    queryClient.invalidateCache('library-index');
    queryClient.invalidateCache('library-series');
    delete structureCache.folders;
    delete structureCache.series;
    await comp.fetchLibrary({ force: true });
}

export interface ILibraryCtx {
    groups: IUltraCompStateStateful<ILibraryGroup[]>;
    indexGroups: IUltraCompStateStateful<ILibraryIndexGroup[]>;
    libraryLoaded: IUltraCompStateStateful<boolean>;
    queryClient: IUltraCompStateStateful<typeof queryClient>;
    searchQuery: IUltraCompStateStateful<string>;
    structure: IUltraCompStateStateful<TLibraryStructure>;
    lastDeleted: IUltraCompStateStateful<{ uid: string } | null>;
    identifyProgress: IUltraCompStateStateful<Extract<
        TIdentifyProgress,
        { type: 'identifying' }
    > | null>;
    setStructure: (structure: TLibraryStructure) => Promise<void>;
    fetchLibrary: (options?: { force?: boolean }) => Promise<void>;
    refreshLibrary: (options?: { silent?: boolean }) => Promise<void>;
    refreshLibraryWithPrompt: () => Promise<void>;
    identifyLibrary: () => Promise<void>;
    deleteFile: (uid: string) => Promise<void>;
    deleteFolder: (uid: string) => Promise<void>;
    createFolder: (folderName: string, parentFolderUid?: string) => Promise<void>;
    moveFile: (fileUid: string, targetFolderUid?: string) => Promise<void>;
    unidentifyFile: (uid: string) => Promise<void>;
    reidentifyAll: () => Promise<void>;
    getLibraryItems: (args: { onlyDir: boolean; uid?: string }) => ILibraryResponseItem[];
    findUidByPath: (absPath: string) => string | undefined;
}

export const LIBRARY_CONTEXT: ILibraryCtx = ultraCompState({
    groups: [] as ILibraryGroup[],

    indexGroups: [] as ILibraryIndexGroup[],

    libraryLoaded: false as boolean,

    queryClient: queryClient,

    searchQuery: '' as string,

    structure: readStoredStructure() as TLibraryStructure,

    lastDeleted: null as { uid: string } | null,

    identifyProgress: null as Extract<TIdentifyProgress, { type: 'identifying' }> | null,

    setStructure: async (comp: ILibraryCtx, structure: TLibraryStructure) => {
        const current = comp.structure.get();
        if (current === structure) return;

        try {
            localStorage.setItem(STRUCTURE_STORAGE_KEY, structure);
        } catch {}

        if (comp.libraryLoaded.get()) structureCache[current] = comp.groups.get();
        const cached = structureCache[structure];

        libraryLoadInFlight = null;
        comp.structure.set(structure);
        comp.libraryLoaded.set(Boolean(cached));
        comp.groups.set(cached ?? []);

        try {
            await comp.fetchLibrary();
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to load library.');
        }
    },

    fetchLibrary: async (comp: ILibraryCtx, options?: { force?: boolean }) => {
        if (options?.force) {
            libraryLoadInFlight = null;
            comp.libraryLoaded.set(false);
        }

        if (comp.libraryLoaded.get() && comp.groups.get().length) return;
        if (libraryLoadInFlight) return libraryLoadInFlight;

        const structure = comp.structure.get();
        const isCurrent = () => comp.structure.get() === structure;

        const loadId = ++libraryLoadId;

        const run: Promise<void> = (async () => {
            try {
                let merged: ILibraryGroup[] = [];

                if (structure === 'folders') {
                    const { data: index } = (await queryClient.fetch(
                        'library-index',
                        getLibraryIndex,
                        LIBRARY_STALE_TIME,
                    )) as { data: ILibraryIndexGroup[] };
                    if (!isCurrent()) return;

                    comp.indexGroups.set(index);

                    merged = index.map((group) => ({
                        uid: group.uid,
                        name: group.name,
                        path: '',
                        entries: [],
                    }));
                    comp.groups.set([...merged]);
                }

                const { data: firstPage } = (await queryClient.fetch(
                    structure === 'series' ? 'library-series' : 'library',
                    () => getLibraryPage({ offset: 0, structure }),
                    LIBRARY_STALE_TIME,
                )) as { data: ILibraryPage };
                if (!isCurrent()) return;

                merged = mergeLibraryPage(merged, firstPage);
                comp.groups.set([...merged]);

                const pageSize = firstPage.limit || LIBRARY_PAGE_SIZE;
                let offset = firstPage.offset + pageSize;
                let hasMore = firstPage.hasMore;

                while (hasMore) {
                    const nextPage = await getLibraryPage({ offset, limit: pageSize, structure });
                    if (!isCurrent()) return;
                    merged = mergeLibraryPage(merged, nextPage);
                    comp.groups.set([...merged]);
                    offset = nextPage.offset + pageSize;
                    hasMore = nextPage.hasMore;
                }

                comp.libraryLoaded.set(true);
            } finally {
                if (libraryLoadId === loadId) libraryLoadInFlight = null;
            }
        })();

        libraryLoadInFlight = run;
        return run;
    },

    refreshLibrary: async (comp: ILibraryCtx, options?: { silent?: boolean }) => {
        try {
            await queryClient.fetch('library-refresh', requestLibraryRefresh, 0);
            queryClient.invalidateCache('library-refresh');
            await reloadLibrary(comp);
            if (!options?.silent) toast.success('Library refreshed');
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to refresh library.');
        }
    },

    refreshLibraryWithPrompt: async (comp: ILibraryCtx) => {
        await comp.refreshLibrary({ silent: true });

        if (comp.groups.get().every((g) => g.entries.length === 0)) {
            toast.success('Library refreshed');
            return;
        }

        const identify = await CONFIRM_MODAL_CTX.confirmDialog({
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

        await comp.identifyLibrary();
    },

    identifyLibrary: async (comp: ILibraryCtx) => {
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
                    await reloadLibrary(comp);
                    toast.success('Library identified');
                    return;
                }

                comp.identifyProgress.set(
                    progress?.type === 'identifying'
                        ? progress
                        : { type: 'identifying', done: 0, total: 0 },
                );
                await new Promise((resolve) => setTimeout(resolve, IDENTIFY_POLL_INTERVAL_MS));
                status = await getIdentifyLibraryStatus();
            }
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to identify library.');
        } finally {
            identifyPolling = false;
            comp.identifyProgress.set(null);
        }
    },

    deleteFile: async (comp: ILibraryCtx, uid: string) => {
        try {
            const data = await requestDeleteFile(uid);
            await reloadLibrary(comp);
            comp.lastDeleted.set({ uid });
            toast.success(data.message || 'File deleted');
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to delete file.');
        }
    },

    deleteFolder: async (comp: ILibraryCtx, uid: string) => {
        try {
            const data = await requestDeleteFolder(uid);
            await reloadLibrary(comp);
            comp.lastDeleted.set({ uid });
            toast.success(data.message || 'Folder deleted');
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to delete folder.');
        }
    },

    createFolder: async (comp: ILibraryCtx, folderName: string, parentFolderUid?: string) => {
        try {
            const data = await requestCreateFolder(folderName, parentFolderUid);
            await reloadLibrary(comp);
            toast.success(data.message || 'Folder created');
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to create folder.');
        }
    },

    moveFile: async (comp: ILibraryCtx, fileUid: string, targetFolderUid?: string) => {
        try {
            const data = await requestMoveFile(fileUid, targetFolderUid);
            await reloadLibrary(comp);
            toast.success(data.message || 'File moved');
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to move file.');
        }
    },

    unidentifyFile: async (comp: ILibraryCtx, uid: string) => {
        try {
            const data = await requestUnidentifyFile(uid);
            await reloadLibrary(comp);
            toast.success(data.message || 'File un-identified');
        } catch (e) {
            toast.error(e instanceof Error ? e.message : 'Failed to un-identify file.');
        }
    },

    reidentifyAll: async (comp: ILibraryCtx) => {
        try {
            const data = await requestReidentifyAllLibrary();
            await reloadLibrary(comp);
            toast.success(data.message || 'Library flagged for re-identification');
        } catch (e) {
            toast.error(
                e instanceof Error ? e.message : 'Failed to flag library for re-identification.',
            );
        }
    },

    getLibraryItems: (
        comp: ILibraryCtx,
        { onlyDir, uid }: { onlyDir: boolean; uid?: string },
    ): ILibraryResponseItem[] => {
        const groups = comp.groups.get();

        if (comp.structure.get() === 'series') {
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

    findUidByPath: (comp: ILibraryCtx, absPath: string): string | undefined => {
        const groups = comp.groups.get();

        const group = groups.find((g) => g.path === absPath);
        if (group) return group.uid;

        for (const g of groups) {
            const entry = g.entries.find((e) => e.did && e.path === absPath);
            if (entry) return entry.uid;
        }

        return undefined;
    },
});
