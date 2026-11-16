import { useMemo } from 'react';
import { useLibraryStore } from '@/stores/library.store';
import { ArrowRightIcon } from '@/icons/arrow-right.icon';
import styles from './reader-next.module.css';

export function ReaderNext({ uid, navigate }: { navigate: (value: string) => void; uid: string }) {
    const { groups, getLibraryItems } = useLibraryStore();

    const nextItem = useMemo(() => {
        const libItems = groups.flatMap((g) => g.entries);
        const item = libItems.find((i) => i.uid === uid);
        const parentItem = libItems.find((i) => i.uid === item?.parentId);
        const folderItems = getLibraryItems({ uid: parentItem?.uid, onlyDir: false });
        const currentIndex = folderItems.findIndex((i) => i.uid === uid);
        if (currentIndex === -1) return undefined;
        return folderItems[currentIndex + 1];
    }, [groups, getLibraryItems, uid]);

    if (!nextItem) return null;

    return (
        <button
            type="button"
            className={styles.button}
            onClick={() => navigate(`/${nextItem.uid}/reader`)}
            aria-label={`Read next: ${nextItem.name}`}
        >
            <span className={styles.text}>
                <span className={styles.label}>Up next</span>
                <span className={styles.title}>{nextItem.name}</span>
            </span>
            <span className={styles.icon}>
                <ArrowRightIcon size={16} color="#0a0a0a" />
            </span>
        </button>
    );
}
