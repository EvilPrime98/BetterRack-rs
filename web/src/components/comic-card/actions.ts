import { UltraComponent } from 'ultra-light-js';
import styles from './comic-card.module.css';
import { MarkAsReadButton } from './mark-as-read-button';
import { DeleteFileButton } from './delete-button';
import { MoveFileButton } from './move-button';
import { RefreshComicButton } from './refresh-comic-button';
import { DownloadComicButton } from './download-button';

export function ComicCardActions({
    uid,
    name,
    onComicRefreshed,
}: {
    uid: string;
    name: string;
    onComicRefreshed: () => void;
}) {
    return UltraComponent({
        component: '<div></div>',
        className: [styles.actions],
        children: [
            MarkAsReadButton({ uid }),
            DownloadComicButton({ uid, name }),
            MoveFileButton({ uid, name }),
            RefreshComicButton({ uid, onRefreshed: onComicRefreshed }),
            DeleteFileButton({ uid }),
        ],
    });
}
