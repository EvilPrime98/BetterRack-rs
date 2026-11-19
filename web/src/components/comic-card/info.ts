import { UltraActivity, UltraComponent, ultraNavigate } from 'ultra-light-js';
import { writerHref } from '@/lib/writer-filter';
import styles from './comic-card.module.css';
import type { WikiComic } from 'better-wiki';
import type { TMetaSource } from '@/library.types';
import { InfoRow } from './info-row';
import { Loader } from '@/components/loader/loader';
import { FandomIcon } from '@/icons/fandom.icon';
import { XmlIcon } from '@/icons/xml.icon';

const metaSourceLabel: Record<TMetaSource, string> = {
    wiki: 'From wiki',
    comicinfo: 'From ComicInfo.xml',
};

const metaSourceIcon: Record<TMetaSource, string> = {
    wiki: FandomIcon(),
    comicinfo: XmlIcon(),
};

export function ComicCardInfo({
    comic,
    subsComic,
    metaSource,
    subsMetaSource,
    isLoading,
    subsIsLoading,
}: {
    comic: () => WikiComic | null;
    subsComic: (fn: (value: WikiComic | null) => void) => () => void;
    metaSource: () => TMetaSource | null;
    subsMetaSource: (fn: (value: TMetaSource | null) => void) => () => void;
    isLoading: () => boolean;
    subsIsLoading: (fn: (value: boolean) => void) => () => void;
}) {
    const updateMetaSourceBadge = ($badge: HTMLElement) => {
        const source = metaSource();
        if (!source) {
            $badge.innerHTML = '';
            $badge.style.display = 'none';
            return;
        }
        $badge.innerHTML = metaSourceIcon[source];
        $badge.title = metaSourceLabel[source];
        $badge.style.display = 'flex';
    };

    return UltraComponent({
        component: '<div></div>',

        className: [styles.info],

        children: [
            Loader({
                mode: {
                    state: isLoading,
                    subscriber: subsIsLoading,
                },
                size: 16,
                className: styles.infoLoader,
            }),

            UltraActivity({
                mode: {
                    state: () => !isLoading() && !comic(),
                    subscriber: [subsIsLoading, subsComic],
                },
                component: `<p class="${styles.infoEmpty}">No information found.</p>`,
            }),

            UltraActivity({
                mode: {
                    state: () => !isLoading() && !!comic(),
                    subscriber: [subsIsLoading, subsComic],
                },
                component: '<div></div>',
                styles: { display: 'contents' },
                children: [
                    UltraComponent({
                        component: `<div class="${styles.metaSourceBadge}"></div>`,
                        onMount: [updateMetaSourceBadge],
                        trigger: [
                            {
                                subscriber: subsMetaSource,
                                triggerFunction: updateMetaSourceBadge,
                            },
                        ],
                    }),

                    InfoRow({
                        subsComic,
                        label: 'Comic',
                        onValueChange: ($span) => {
                            $span.textContent = comic()?.title || '';
                        },
                    }),

                    InfoRow({
                        subsComic,
                        label: 'Volume',
                        onValueChange: ($span) => {
                            $span.textContent = comic()?.volume || '';
                        },
                    }),

                    InfoRow({
                        subsComic,
                        label: 'Issue',
                        onValueChange: ($span) => {
                            $span.textContent = comic()?.issue || '';
                        },
                    }),

                    InfoRow({
                        subsComic,
                        label: 'Year',
                        onValueChange: ($span) => {
                            $span.textContent = comic()?.releaseDate?.releaseYear || '';
                        },
                    }),

                    InfoRow({
                        subsComic,
                        label: 'Writer',
                        onValueChange: ($span) => {
                            renderWriters($span, comic()?.credits?.writers || []);
                        },
                    }),

                    InfoRow({
                        subsComic,
                        label: 'Artist',
                        onValueChange: ($span) => {
                            $span.textContent = (comic()?.credits?.artists || []).join(', ');
                        },
                    }),

                    InfoRow({
                        subsComic,
                        label: 'Released',
                        onValueChange: ($span) => {
                            const releaseDate = comic()?.releaseDate;
                            const { releaseMonth, releaseDay, releaseYear } = releaseDate || {};
                            $span.textContent =
                                releaseMonth && releaseDay && releaseYear
                                    ? [releaseMonth, releaseDay]
                                          .map((n) => n.padStart(2, '0'))
                                          .join('/') + `/${releaseYear}`
                                    : '';
                        },
                    }),
                ],
            }),
        ],
    });
}

function renderWriters($span: HTMLElement, writers: string[]) {
    const nodes: (Node | string)[] = [];
    writers.forEach((writer, i) => {
        if (i > 0) nodes.push(', ');
        const $link = document.createElement('a');
        $link.href = writerHref(writer);
        $link.textContent = writer;
        $link.addEventListener('click', (e) => {
            e.preventDefault();
            e.stopPropagation();
            ultraNavigate({ href: writerHref(writer) });
        });
        nodes.push($link);
    });
    $span.replaceChildren(...nodes);
}
