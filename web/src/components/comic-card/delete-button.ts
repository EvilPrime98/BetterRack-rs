import { UltraComponent } from 'ultra-light-js';
import styles from './delete-button.module.css';
import { TrashIcon } from '@/icons/trash.icon';
import { LIBRARY_CONTEXT } from '@/context/library.context';
import { CONFIRM_MODAL_CTX } from '@/context/confirm-modal.context';

export function DeleteFileButton({ uid }: { uid: string }) {
    const onClick = async (e: Event) => {
        e.stopPropagation();
        e.preventDefault();
        const confirmed = await CONFIRM_MODAL_CTX.confirmDialog({
            title: 'Delete comic?',
            message: 'This will remove the comic from your library. This cannot be undone.',
            confirmLabel: 'Delete',
        });
        if (!confirmed) return;
        LIBRARY_CONTEXT.deleteFile(uid);
    };

    return UltraComponent({
        component: '<span></span>',
        className: [styles.deleteButton],
        attributes: {
            type: 'button',
            'aria-label': 'Delete this comic from the library',
        },
        eventHandler: {
            click: onClick,
        },
        children: [TrashIcon({ size: 20 })],
    });
}
