import { UltraComponent, UltraLink, ultraState } from 'ultra-light-js';
import type { ILibraryResponseItem } from '../../library.types';
import { NO_IMAGE_URL } from '../../data';
import { type WikiComic } from 'better-wiki';
import styles from './comic-card.module.css';
import { ImageGen } from '../image-generic/image-generic';
import { API_URL } from '@/services/library.service';
import { withAuthQuery } from '@/services/server-config.service';

export function ComicCardCover({
    item,
    comic,
    subsComic,
    coverVersion,
    subsCoverVersion,
}: {
    item: ILibraryResponseItem;
    comic: () => WikiComic | null;
    subsComic: (fn: (value: WikiComic | null) => void) => () => void;
    coverVersion: () => number;
    subsCoverVersion: (fn: (value: number) => void) => () => void;
}) {
    const readerHref = `/${item.uid}/reader`;
    const [loaded, setIsLoaded, subsIsLoaded] = ultraState(false);

    const coverSrc = () => {
        const versionQuery = coverVersion() ? `?v=${coverVersion()}` : '';
        return withAuthQuery(`${API_URL}/api/thumbnail/${item.uid}${versionQuery}`);
    };

    const onCoverMount = ($img: HTMLElement) => {
        ($img as HTMLImageElement).src = coverSrc();
    };

    const onCoverVersionChange = ($img: HTMLElement) => {
        setIsLoaded(false);
        ($img as HTMLImageElement).src = coverSrc();
    };

    const onEventChange = ($span: HTMLElement) => {
        $span.textContent = comic()?.event || '';
    };

    const onCoverError = (e: Event) => {
        const $img = e.currentTarget as HTMLImageElement;
        if ($img.src === NO_IMAGE_URL) {
            setIsLoaded(true);
            return;
        }
        $img.src = NO_IMAGE_URL;
    };

    const onIssueBadgeChange = ($span: HTMLElement) => {
        const issue = comic()?.issue;
        $span.textContent = issue ? `#${issue}` : '';
    };

    return UltraLink({
        href: readerHref,

        trigger: [
            {
                subscriber: subsIsLoaded,
                triggerFunction: ($link: HTMLElement) => {
                    $link.classList.toggle(styles.loaded, loaded());
                },
            },
        ],

        children: [
            ImageGen({
                attributes: {
                    alt: item.name,
                    title: item.name,
                },
                eventHandler: {
                    load: () => setIsLoaded(true),
                    error: onCoverError,
                },
                onMount: [onCoverMount],
                trigger: [
                    {
                        subscriber: subsCoverVersion,
                        triggerFunction: onCoverVersionChange,
                    },
                ],
            }),

            UltraComponent({
                component: `<span class="${styles.issueBadge}"></span>`,
                onMount: [onIssueBadgeChange],
                trigger: [
                    {
                        subscriber: subsComic,
                        triggerFunction: onIssueBadgeChange,
                    },
                ],
            }),

            UltraComponent({
                component: `<span class="${styles.eventBadge}"></span>`,
                onMount: [onEventChange],
                trigger: [
                    {
                        subscriber: subsComic,
                        triggerFunction: onEventChange,
                    },
                ],
            }),
        ],
    });
}
