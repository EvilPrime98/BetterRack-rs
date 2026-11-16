import { useState, type MouseEvent } from 'react';
import styles from './refresh-comic-button.module.css';
import { RefreshIcon } from '@/icons/refresh-icon';
import { retryThumbnail, reidentifyFile } from '@/services/library.service';
import { useComicIdentStore } from '@/stores/comicIdent.store';

export function RefreshComicButton({ uid, onRefreshed }: { uid: string; onRefreshed: () => void }) {
    const [isRefreshing, setIsRefreshing] = useState(false);

    const refresh = () => {
        if (isRefreshing) return;
        setIsRefreshing(true);
        Promise.allSettled([retryThumbnail(uid), reidentifyFile(uid)])
            .then(([, identifyResult]) => {
                if (identifyResult.status !== 'fulfilled') return;
                const { identified, comic, metaSource } = identifyResult.value;
                if (identified && comic && metaSource) {
                    useComicIdentStore.getState().setLastIdentified({ uid, comic, metaSource });
                } else {
                    useComicIdentStore.getState().setLastUnidentified({ uid });
                }
            })
            .finally(() => {
                setIsRefreshing(false);
                onRefreshed();
            });
    };

    const onClick = (e: MouseEvent) => {
        e.stopPropagation();
        e.preventDefault();
        refresh();
    };

    return (
        <button
            type="button"
            className={styles.refreshComicButton}
            aria-label="Regenerate the cover thumbnail and re-identify this comic"
            title="Regenerate the cover thumbnail and re-identify this comic"
            aria-busy={isRefreshing}
            onClick={onClick}
        >
            <RefreshIcon size={16} />
        </button>
    );
}
