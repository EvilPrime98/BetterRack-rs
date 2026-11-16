import { useRef, useState } from 'react';
import { ImageGen } from '@/components/image-generic/image-generic';
import styles from '@/components/comic-card/comic-card.module.css';

export function FolderStackCard({ cover }: { cover: string; isRead: boolean }) {
    const anchorRef = useRef<HTMLDivElement>(null);
    const [loaded, setLoaded] = useState(false);
    const onImageLoad = () => setLoaded(true);

    return (
        <article
            className={[styles.comicCard, styles.stackLayer].join(' ')}
            style={{ padding: '0' }}
        >
            <div className={styles.cover}>
                <div className={styles.board} />

                <div
                    ref={anchorRef}
                    className={[styles.frame, loaded ? styles.loaded : '']
                        .filter(Boolean)
                        .join(' ')}
                >
                    <ImageGen src={cover} onLoad={onImageLoad} />
                </div>

                <div className={styles.bagOverlay} />
            </div>
        </article>
    );
}
