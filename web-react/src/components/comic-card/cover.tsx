import { useState, type SyntheticEvent } from 'react';
import { Link } from 'react-router-dom';
import type { ILibraryResponseItem } from '@/library.types';
import { NO_IMAGE_URL } from '@/data';
import type { WikiComic } from 'better-wiki';
import styles from './comic-card.module.css';
import { ImageGen } from '@/components/image-generic/image-generic';
import { API_URL } from '@/services/library.service';
import { withAuthQuery } from '@/services/server-config.service';

export function ComicCardCover({
    item,
    comic,
    coverVersion,
}: {
    item: ILibraryResponseItem;
    comic: WikiComic | null;
    coverVersion: number;
}) {
    const readerHref = `/${item.uid}/reader`;
    const versionQuery = coverVersion ? `?v=${coverVersion}` : '';
    const coverSrc = withAuthQuery(`${API_URL}/api/thumbnail/${item.uid}${versionQuery}`);

    const [loaded, setLoaded] = useState(false);
    const [imgSrc, setImgSrc] = useState(coverSrc);

    const onCoverError = (e: SyntheticEvent<HTMLImageElement>) => {
        const $img = e.currentTarget;
        if ($img.src === NO_IMAGE_URL) {
            setLoaded(true);
            return;
        }
        setImgSrc(NO_IMAGE_URL);
    };

    return (
        <Link
            to={readerHref}
            className={[styles.frame, loaded ? styles.loaded : ''].filter(Boolean).join(' ')}
        >
            <ImageGen
                src={imgSrc}
                alt={item.name}
                title={item.name}
                onLoad={() => setLoaded(true)}
                onError={onCoverError}
            />

            <span className={styles.issueBadge}>{comic?.issue ? `#${comic.issue}` : ''}</span>

            <span className={styles.eventBadge}>{comic?.event || ''}</span>
        </Link>
    );
}
