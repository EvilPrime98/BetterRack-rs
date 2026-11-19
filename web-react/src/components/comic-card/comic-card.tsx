import { memo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import styles from './comic-card.module.css';
import { ReadBar } from '@/components/read-bar/read-bar';
import { ComicRating } from './rating';
import type { ILibraryResponseItem } from '@/library.types';
import { useComicsTypeStore } from '@/stores/comicsTypes.store';
import { ComicCardTitle } from './title';
import { ComicCardInfo } from './info';
import { ComicCardCover } from './cover';
import {
    matchesReadFilter as readFilterMatches,
    useReadTypesContext,
} from '@/context/ReadTypesContext.hooks';
import { useComicCacheStore } from '@/stores/comicCache.store';
import { useComicIdentification } from '@/hooks/useComicIdentification';
import { IdentifyButton } from './identify-button';
import { ComicCardActions } from './actions';
import { CrButton } from '@/components/cr-button/cr-button';
import { COMIC_FILTERS, type IComicFilters } from '@/library.types';

export const ComicCard = memo(function ComicCard({
    item,
    filters,
    forceCover = false,
}: {
    filters?: IComicFilters;
    item: ILibraryResponseItem;
    forceCover?: boolean;
}) {
    const navigate = useNavigate();
    const { comic, metaSource, isLoadingInfo, articleRef } = useComicIdentification(item, filters);
    const [coverVersion, setCoverVersion] = useState(0);
    const readerHref = `/${item.uid}/reader`;
    const storedType = useComicsTypeStore((s) => s.type);
    const comicsType = forceCover ? 'cover' : storedType;
    const { type: readFilter } = useReadTypesContext();
    const itemCache = useComicCacheStore((s) => s.cache[item.uid] ?? null);
    const isRead = itemCache?.read === true;
    const readPer = itemCache ? (itemCache.read === true ? 100 : (itemCache.readPer ?? 0)) : 0;

    const isVisible = (() => {
        const writerFilter = filters?.[COMIC_FILTERS.writer];
        const currReadPer = itemCache?.readPer ?? 0;

        const matchesWriter =
            !writerFilter || comic?.credits.writers?.some((w) => w === writerFilter) === true;

        return forceCover || (matchesWriter && readFilterMatches(readFilter, currReadPer));
    })();

    const articleClassName = [
        styles.comicCard,
        comicsType === 'detail' ? styles.detailMode : '',
        isRead ? styles.isRead : '',
    ]
        .filter(Boolean)
        .join(' ');

    return (
        <article
            ref={articleRef}
            className={articleClassName}
            style={{ display: isVisible ? undefined : 'none' }}
        >
            <div className={styles.cover}>
                <div className={styles.board} />

                <ComicCardCover
                    key={coverVersion}
                    coverVersion={coverVersion}
                    comic={comic}
                    item={item}
                />

                <div className={styles.bagOverlay} />

                <ReadBar readPercentage={readPer} />
            </div>

            <div className={styles.details}>
                <ComicCardTitle comic={comic} item={item} readerHref={readerHref} />

                <div style={{ display: 'flex', gap: '5px' }}>
                    <IdentifyButton
                        uid={item.uid}
                        style={{ display: comicsType === 'detail' ? undefined : 'none' }}
                    />

                    <CrButton
                        variant="orange"
                        text="Read"
                        style={{ display: comicsType === 'detail' ? undefined : 'none' }}
                        onClick={() => navigate(readerHref)}
                    />
                </div>

                <ComicCardInfo comic={comic} metaSource={metaSource} isLoading={isLoadingInfo} />

                <div className={styles.actionsBlock}>
                    <ComicRating uid={item.uid} />
                    <ComicCardActions
                        uid={item.uid}
                        name={item.name}
                        onComicRefreshed={() => setCoverVersion(Date.now())}
                    />
                </div>
            </div>
        </article>
    );
});
