import type { MouseEvent } from 'react';
import styles from './delete-button.module.css';
import { TrashIcon } from '@/icons/trash.icon';
import { useLibraryStore } from '@/stores/library.store';
import { useConfirmModalStore } from '@/stores/confirmModal.store';

export function DeleteFileButton({ uid }: { uid: string }) {
    const onClick = async (e: MouseEvent) => {
        e.stopPropagation();
        e.preventDefault();
        const confirmed = await useConfirmModalStore.getState().confirmDialog({
            title: 'Delete comic?',
            message: 'This will remove the comic from your library. This cannot be undone.',
            confirmLabel: 'Delete',
        });
        if (!confirmed) return;
        useLibraryStore.getState().deleteFile(uid);
    };

    return (
        <button
            type="button"
            className={styles.deleteButton}
            aria-label="Delete this comic from the library"
            title="Delete this comic from the library"
            onClick={onClick}
        >
            <TrashIcon size={20} />
        </button>
    );
}
