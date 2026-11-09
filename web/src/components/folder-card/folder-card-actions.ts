import { UltraComponent } from 'ultra-light-js';
import styles from './folder-card.module.css';
import { TrashIcon } from '@/icons/trash.icon';
import { MoveFileButton } from '@/components/comic-card/move-button';
import { CONFIRM_MODAL_CTX } from '@/context/confirm-modal.context';
import { LIBRARY_CONTEXT } from '@/context/library.context';

export function FolderCardActions({ uid, title }: { uid: string; title: string }) {
    const onDeleteClick = async (e: Event) => {
        e.stopPropagation();
        e.preventDefault();
        const confirmed = await CONFIRM_MODAL_CTX.confirmDialog({
            title: 'Delete folder?',
            message: `This will remove "${title}" and everything inside it. This cannot be undone.`,
            confirmLabel: 'Delete',
        });
        if (!confirmed) return;
        LIBRARY_CONTEXT.deleteFolder(uid);
    };

    return UltraComponent({
        component: '<div></div>',
        className: [styles.folderActions],
        children: [
            MoveFileButton({ uid, name: title }),
            UltraComponent({
                component: '<span></span>',
                className: [styles.folderActionButton, styles.folderDeleteButton],
                attributes: {
                    type: 'button',
                    'aria-label': 'Delete this folder',
                },
                eventHandler: { click: onDeleteClick },
                children: [TrashIcon({ size: 14 })],
            }),
        ],
    });
}
