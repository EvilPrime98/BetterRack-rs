import { useEffect, useMemo, useState } from 'react';
import styles from './move-file-modal.module.css';
import { FolderIcon } from '@/icons/folder.icon';
import { BRCheckbox } from '@/components/br-checkbox/br-checkbox';
import { useLibraryStore } from '@/stores/library.store';
import { useMoveFileModalContext } from '@/context/MoveFileModalContext.hooks';
import type { ILibraryGroup } from '@/library.types';

function buildFolderPath(uid: string, groups: ILibraryGroup[]): string {
    const group = groups.find((g) => g.uid === uid);
    if (group) return group.name;

    const byUid = new Map<string, { uid: string; name: string; parentId?: string }>();
    const ownerGroup = new Map<string, ILibraryGroup>();

    groups.forEach((g) =>
        g.entries.forEach((entry) => {
            byUid.set(entry.uid, entry);
            ownerGroup.set(entry.uid, g);
        }),
    );

    const names: string[] = [];
    let current = byUid.get(uid);

    while (current) {
        names.unshift(current.name);
        if (!current.parentId) {
            const owner = ownerGroup.get(current.uid);
            if (owner) names.unshift(owner.name);
            break;
        }
        current = byUid.get(current.parentId);
    }

    return names.join(' / ');
}

export function MoveFileModal() {
    const { isVisible } = useMoveFileModalContext();

    return isVisible ? <MoveFileModalContent /> : null;
}

function MoveFileModalContent() {
    const { fileUid, closeMoveFileModal, selectMoveTarget } = useMoveFileModalContext();
    const groups = useLibraryStore((s) => s.groups);
    // getLibraryItems() builds a fresh array every call — selecting it directly (rather than
    // deriving it via useMemo off the stable `groups` reference) makes every render produce a
    // "changed" snapshot, which triggers an infinite update loop under useSyncExternalStore.
    const folders = useMemo(() => {
        const allItems = useLibraryStore.getState().getLibraryItems({ onlyDir: false });
        // The entry to move can be a folder. A folder cannot move into itself
        // or into a folder nested under it.
        const excluded = new Set<string>([fileUid]);
        for (let added = true; added;) {
            added = false;
            for (const item of allItems) {
                if (item.parentId && excluded.has(item.parentId) && !excluded.has(item.uid)) {
                    excluded.add(item.uid);
                    added = true;
                }
            }
        }
        return useLibraryStore
            .getState()
            .getLibraryItems({ onlyDir: true })
            .filter((folder) => !excluded.has(folder.uid));
    }, [groups, fileUid]);

    const [query, setQuery] = useState('');
    const [showSubfolders, setShowSubfolders] = useState(true);

    const destinations = useMemo(
        () =>
            folders
                .filter((folder) => showSubfolders || !folder.parentId)
                .map((folder) => ({ uid: folder.uid, path: buildFolderPath(folder.uid, groups) })),
        [folders, groups, showSubfolders],
    );

    const normalizedQuery = query.trim().toLowerCase();
    const filteredDestinations = normalizedQuery
        ? destinations.filter((d) => d.path.toLowerCase().includes(normalizedQuery))
        : destinations;
    const showRoot = !normalizedQuery || 'library root'.includes(normalizedQuery);

    const cancel = () => closeMoveFileModal();

    useEffect(() => {
        const onKeydown = (e: KeyboardEvent) => {
            if (e.key === 'Escape') closeMoveFileModal();
        };
        document.addEventListener('keydown', onKeydown);
        return () => document.removeEventListener('keydown', onKeydown);
    }, [closeMoveFileModal]);

    return (
        <div className={styles.overlay}>
            <button
                type="button"
                className={styles.backdrop}
                aria-label="Close dialog"
                onClick={cancel}
            />

            <div
                role="dialog"
                aria-modal="true"
                aria-label="Move to another folder"
                className={styles.modal}
            >
                <p className={styles.title}>Move to: </p>

                <div className={styles.searchRow}>
                    <input
                        type="text"
                        autoFocus
                        className={styles.search}
                        placeholder="Search folders..."
                        aria-label="Search folders"
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                    />

                    <BRCheckbox
                        className={styles.checkbox}
                        text="Sub-folders"
                        checked={showSubfolders}
                        onChange={(e) => setShowSubfolders(e.target.checked)}
                    />
                </div>

                <ul className={styles.list}>
                    {showRoot && (
                        <li>
                            <button
                                type="button"
                                className={styles.item}
                                title="Library root"
                                onClick={() => selectMoveTarget(undefined)}
                            >
                                <FolderIcon size={14} color="#34c3d1" />
                                <span>Library root</span>
                            </button>
                        </li>
                    )}

                    {filteredDestinations.map(({ uid, path }) => (
                        <li key={uid}>
                            <button
                                type="button"
                                className={styles.item}
                                title={path}
                                onClick={() => selectMoveTarget(uid)}
                            >
                                <FolderIcon size={14} color="#c7c7c7" />
                                <span>{path}</span>
                            </button>
                        </li>
                    ))}

                    {!showRoot && !filteredDestinations.length && (
                        <li className={styles.empty}>
                            {folders.length ? 'No folders match your search.' : 'No folders yet.'}
                        </li>
                    )}
                </ul>
            </div>
        </div>
    );
}
