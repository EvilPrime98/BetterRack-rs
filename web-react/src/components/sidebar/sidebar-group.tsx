import { useId, type MouseEvent } from 'react';
import styles from './sidebar.module.css';
import { useLibraryStore } from '@/stores/library.store';
import { useSidebarStore } from '@/stores/sidebar.store';
import { useNewFolderModalContext } from '@/context/NewFolderModalContext.hooks';
import { SideBarElement } from './sider-bar-element';
import { FolderIcon } from '@/icons/folder.icon';
import { FolderPlusIcon } from '@/icons/folder-plus.icon';
import { ChevronDownIcon } from '@/icons/chevron.icon';
import type { ILibraryGroup } from '@/library.types';

export function SideBarGroup({ group }: { group: ILibraryGroup }) {
    const listId = useId();
    const isExpanded = useSidebarStore((s) => !!s.expandedGroups[group.uid]);
    const toggleGroup = useSidebarStore((s) => s.toggleGroup);
    const getLibraryItems = useLibraryStore((s) => s.getLibraryItems);
    const { openNewFolderModal } = useNewFolderModalContext();

    function toggle() {
        toggleGroup(group.uid);
    }

    function onNewFolder(e: MouseEvent) {
        e.stopPropagation();
        openNewFolderModal(group.uid);
    }

    const entries = isExpanded ? getLibraryItems({ onlyDir: true, uid: group.uid }) : [];

    return (
        <div className={styles.group}>
            <div className={styles.groupHeader}>
                <button
                    type="button"
                    className={styles.groupHeaderToggle}
                    aria-expanded={isExpanded}
                    aria-controls={listId}
                    title={group.name}
                    onClick={toggle}
                >
                    <span className={styles.groupHeaderIcon} aria-hidden="true">
                        <FolderIcon size={16} />
                    </span>
                    <span className={styles.groupHeaderLabel}>{group.name}</span>
                    <span
                        className={[styles.chevron, isExpanded ? styles.chevronExpanded : '']
                            .filter(Boolean)
                            .join(' ')}
                        aria-hidden="true"
                    >
                        <ChevronDownIcon size={14} />
                    </span>
                </button>
                <button
                    type="button"
                    className={styles.groupHeaderAction}
                    aria-label={`New folder in ${group.name}`}
                    title={`New folder in ${group.name}`}
                    onClick={onNewFolder}
                >
                    <span aria-hidden="true">
                        <FolderPlusIcon size={14} />
                    </span>
                </button>
            </div>

            <div
                id={listId}
                role="group"
                aria-label={group.name}
                className={styles.groupList}
                hidden={!isExpanded}
            >
                {entries.map((item) => (
                    <SideBarElement key={item.uid} item={item} />
                ))}
            </div>
        </div>
    );
}
