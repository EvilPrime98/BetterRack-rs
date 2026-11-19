import { useState } from 'react';
import styles from './comic-card.module.css';
import { StarComponent } from '@/components/star-component/star-component';
import { useComicCacheStore } from '@/stores/comicCache.store';

export function ComicRating({ uid }: { uid: string }) {
    const rating = useComicCacheStore((s) => s.cache[uid]?.rating || 0);
    const [preview, setPreview] = useState<number | null>(null);

    const changeRating = (newRating: number) => {
        useComicCacheStore.getState().setCacheById(uid, { rating: newRating });
    };

    // While hovering, the whole row previews the rating a click would set;
    // otherwise it shows the committed rating.
    const displayRating = Math.max(0, Math.min(5, preview ?? rating));

    const stars = [];
    for (let i = 0; i < 5; i++) {
        stars.push(
            <StarComponent
                key={i}
                weight={i + 1}
                changeRating={changeRating}
                onHover={setPreview}
                initialFill={Math.max(0, Math.min(1, displayRating - i))}
            />,
        );
    }

    return (
        <div className={styles.rating} onMouseLeave={() => setPreview(null)}>
            {stars}
        </div>
    );
}
