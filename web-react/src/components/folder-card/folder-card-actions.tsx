import { TrashIcon } from '@/icons/trash.icon';
import { MoveFileButton } from '../comic-card/move-button';
import styles from './folder-card.module.css';
import { useConfirmModalStore } from '@/stores/confirmModal.store';
import { useLibraryStore } from '@/stores/library.store';

export function FolderCardActions({ uid, title }: { uid: string; title: string }) {
    const onDeleteClick = async (e: React.MouseEvent) => {
        e.stopPropagation();
        e.preventDefault();
        const confirmed = await useConfirmModalStore.getState().confirmDialog({
            title: 'Delete folder?',
            message: `This will remove "${title}" and everything inside it. This cannot be undone.`,
            confirmLabel: 'Delete',
        });
        if (!confirmed) return;
        useLibraryStore.getState().deleteFolder(uid);
    };

    return (
        <div className={styles.folderActions}>
            <MoveFileButton uid={uid} name={title} />
            <button
                type="button"
                className={[styles.folderActionButton, styles.folderDeleteButton].join(' ')}
                aria-label="Delete this folder"
                onClick={onDeleteClick}
            >
                <TrashIcon size={14} />
            </button>
        </div>
    );
}
