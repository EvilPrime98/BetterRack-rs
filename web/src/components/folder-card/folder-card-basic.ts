import { FolderIcon } from '@/icons/folder.icon';
import { UltraComponent } from 'ultra-light-js';
import styles from './folder-card.module.css';

export function FolderCardBasic({ title }: { title: string }) {
    return UltraComponent({
        component: '<span></span>',

        className: [styles.cover, styles.basicFolderCard],

        children: [
            FolderIcon({
                size: 40,
                color: '#34c3d1',
            }),

            `<span>${title}</span>`,
        ],
    });
}
