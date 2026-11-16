import { FolderIcon } from '@/icons/folder.icon';
import styles from './folder-card.module.css';

export function FolderCardBasic({ title }: { title: string }) {
    return (
        <span className={[styles.cover, styles.basicFolderCard].join(' ')}>
            <FolderIcon size={40} color="#34c3d1" />

            <span>{title}</span>
        </span>
    );
}
