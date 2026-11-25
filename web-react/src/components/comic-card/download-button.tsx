import { useState, type MouseEvent } from 'react';
import styles from './download-button.module.css';
import { DownloadIcon } from '@/icons/download.icon';
import { downloadComic } from '@/services/download.service';
import { toast } from '@/services/toast.service';

export function DownloadComicButton({ uid, name }: { uid: string; name: string }) {
    const [isDownloading, setIsDownloading] = useState(false);

    const onClick = (e: MouseEvent) => {
        e.stopPropagation();
        e.preventDefault();
        if (isDownloading) return;
        setIsDownloading(true);
        downloadComic(uid, name)
            .then((savedPath) => {
                if (savedPath) toast.success(`Saved to ${savedPath}`);
            })
            .catch((e) => {
                toast.error(
                    e instanceof Error ? e.message : 'There was an error downloading the comic.',
                );
            })
            .finally(() => setIsDownloading(false));
    };

    return (
        <button
            type="button"
            className={styles.downloadComicButton}
            aria-label="Download this comic to your device"
            title="Download this comic to your device"
            aria-busy={isDownloading}
            onClick={onClick}
        >
            <DownloadIcon size={16} />
        </button>
    );
}
