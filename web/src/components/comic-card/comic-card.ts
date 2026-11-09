import { UltraActivity, UltraComponent, ultraNavigate, ultraState } from 'ultra-light-js';
import type { WikiComic } from 'better-wiki';
import styles from './comic-card.module.css';
import { ReadBar } from '@/components/read-bar/read-bar';
import { ComicRating } from './rating';
import { type IComicLSCache, type ILibraryResponseItem } from '@/library.types';
import { COMICS_TYPE_CTX } from '@/context/comics-types.context';
import { ComicCardTitle } from './title';
import { ComicCardInfo } from './info';
import { ComicCardCover } from './cover';
import { matchesReadFilter, READ_TYPES_CTX } from '@/context/read-types.context';
import { COMIC_CACHE_CONTEXT } from '@/context/comic-cache.context';
import { COMIC_IDENT_CTX } from '@/context/identifer-modal.context';
import { IdentifyButton } from './identify-button';
import { ComicCardActions } from './actions';
import { CrButton } from '../cr-button/cr-button';
import { identifyLibraryEntry } from '@/services/library.service';

export function ComicCard({ item }: { item: ILibraryResponseItem }) {
    const [comic, setComic, subsComic] = ultraState<WikiComic | null>(item.comic ?? null);
    const [metaSource, setMetaSource, subsMetaSource] = ultraState(item.metaSource);
    const [identified, setIdentified, subsIdentified] = ultraState(item.identified !== false);
    const [isLoadingInfo, setIsLoadingInfo, subsIsLoadingInfo] = ultraState(
        item.identified === undefined,
    );
    const [coverVersion, setCoverVersion, subsCoverVersion] = ultraState(0);

    const [itemCache, setItemCache, subsItemCache] = ultraState<IComicLSCache | null>(
        COMIC_CACHE_CONTEXT.getCacheById(item.uid) || null,
    );

    const readerHref = `/${item.uid}/reader`;

    const getIsRead = () => itemCache()?.read === true;

    const getReadPer = () => {
        const cache = itemCache();
        return cache ? (cache.read === true ? 100 : (cache.readPer ?? 0)) : 0;
    };

    const onCardTypeChange = ($article: HTMLElement) => {
        $article.classList.toggle(styles.detailMode, COMICS_TYPE_CTX.type.get() === 'detail');
    };

    const onReadStateChange = ($article: HTMLElement) => {
        $article.classList.toggle(styles.isRead, getIsRead());
    };

    const isVisible = () => {
        return matchesReadFilter(READ_TYPES_CTX.type.get(), itemCache()?.readPer || 0);
    };

    COMIC_CACHE_CONTEXT.subscribeById(item.uid, (entry) => setItemCache(entry || null));

    COMIC_IDENT_CTX.lastIdentified.subscribe((entry) => {
        if (entry?.uid !== item.uid) return;
        setComic(entry.comic);
        setMetaSource(entry.metaSource);
        setIdentified(true);
        setIsLoadingInfo(false);
    });

    const onCardMount = ($article: HTMLElement) => {
        if (item.identified !== undefined) return undefined;

        const observer = new IntersectionObserver(
            (entries) => {
                if (!entries.some((e) => e.isIntersecting)) return;
                observer.disconnect();
                setIsLoadingInfo(true);
                identifyLibraryEntry(item.uid)
                    .then((resolved) => {
                        setComic(resolved.comic ?? null);
                        setMetaSource(resolved.metaSource);
                        setIdentified(resolved.identified === true);
                    })
                    .catch(() => {})
                    .finally(() => setIsLoadingInfo(false));
            },
            { rootMargin: '200px' },
        );

        observer.observe($article);

        return () => observer.disconnect();
    };

    return UltraActivity({
        mode: {
            state: isVisible,
            subscriber: [READ_TYPES_CTX.type.subscribe, subsItemCache],
        },

        component: '<article></article>',

        onMount: [onCardMount],

        className: [
            styles.comicCard,
            ...(COMICS_TYPE_CTX.type.get() === 'detail' ? [styles.detailMode] : []),
            ...(getIsRead() ? [styles.isRead] : []),
        ],

        trigger: [
            {
                subscriber: COMICS_TYPE_CTX.type.subscribe,
                triggerFunction: onCardTypeChange,
            },
            {
                subscriber: subsItemCache,
                triggerFunction: onReadStateChange,
            },
        ],

        children: [
            UltraComponent({
                component: '<div></div>',

                className: [styles.cover],

                children: [
                    UltraComponent({
                        component: `<div class="${styles.board}"></div>`,
                    }),

                    ComicCardCover({
                        comic,
                        subsComic,
                        coverVersion,
                        subsCoverVersion,
                        item,
                    }),

                    UltraComponent({
                        component: `<div class="${styles.bagOverlay}"></div>`,
                    }),

                    UltraActivity({
                        component: UltraComponent({
                            component: '<div></div>',
                            className: [styles.identifyOverlay],
                            children: [IdentifyButton({ uid: item.uid })],
                        }),
                        mode: {
                            state: () => COMICS_TYPE_CTX.type.get() !== 'detail' && !identified(),
                            subscriber: [COMICS_TYPE_CTX.type.subscribe, subsIdentified],
                        },
                    }),

                    ReadBar({
                        getReadPercentage: getReadPer,
                        subsReadPercentage: subsItemCache,
                    }),
                ],
            }),

            UltraComponent({
                component: '<div></div>',

                className: [styles.details],

                children: [
                    ComicCardTitle({
                        comic,
                        subsComic,
                        readerHref,
                        item,
                    }),

                    UltraComponent({
                        component: '<div></div>',
                        styles: {
                            display: 'flex',
                            gap: '5px',
                        },
                        children: [
                            UltraActivity({
                                component: IdentifyButton({ uid: item.uid }),
                                mode: {
                                    state: () => COMICS_TYPE_CTX.type.get() === 'detail',
                                    subscriber: COMICS_TYPE_CTX.type.subscribe,
                                },
                            }),
                            UltraActivity({
                                component: CrButton({
                                    variant: 'orange',
                                    text: 'Read',
                                    eventHandler: {
                                        click: () => ultraNavigate({ href: `/${item.uid}/reader` }),
                                    },
                                }),
                                mode: {
                                    state: () => COMICS_TYPE_CTX.type.get() === 'detail',
                                    subscriber: COMICS_TYPE_CTX.type.subscribe,
                                },
                            }),
                        ],
                    }),

                    ComicCardInfo({
                        comic,
                        subsComic,
                        metaSource,
                        subsMetaSource,
                        isLoading: isLoadingInfo,
                        subsIsLoading: subsIsLoadingInfo,
                    }),

                    UltraComponent({
                        component: '<div></div>',
                        className: [styles.actionsBlock],
                        children: [
                            ComicRating({
                                uid: item.uid,
                            }),

                            ComicCardActions({
                                uid: item.uid,
                                name: item.name,
                                onComicRefreshed: () => setCoverVersion(Date.now()),
                            }),
                        ],
                    }),
                ],
            }),
        ],
    });
}
