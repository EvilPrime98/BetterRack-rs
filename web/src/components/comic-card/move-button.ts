import { UltraComponent } from 'ultra-light-js';
import styles from './move-button.module.css';
import { FolderIcon } from '@/icons/folder.icon';
import { MOVE_FILE_MODAL_CTX } from '@/context/move-file-modal.context';

export function MoveFileButton({ uid, name }: { uid: string; name: string }) {
    const onClick = (e: Event) => {
        e.stopPropagation();
        e.preventDefault();
        MOVE_FILE_MODAL_CTX.openMoveFileModal(uid, name);
    };

    return UltraComponent({
        component: '<span></span>',
        className: [styles.moveButton],
        attributes: {
            type: 'button',
            'aria-label': 'Move to another folder',
        },
        eventHandler: {
            click: onClick,
        },
        children: [FolderIcon({ size: 18 })],
    });
}
