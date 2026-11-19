import { UltraComponent, UltraLink } from 'ultra-light-js';
import styles from './comic-card.module.css';
import type { ILibraryResponseItem } from '../../library.types';
import type { WikiComic } from 'better-wiki';
import { COMICS_TYPE_CTX } from '../../context/comics-types.context';

const displayable = (toEvaluate: string) => {
    return !['', 'undefined'].includes(toEvaluate);
};

export function ComicCardTitle({
    item,
    comic,
    subsComic,
    readerHref,
}: {
    item: ILibraryResponseItem;
    comic: () => WikiComic | null;
    subsComic: (fn: (value: WikiComic | null) => void) => () => void;
    readerHref: string;
}) {
    const onTitleChange = ($p: HTMLElement) => {
        const currType = COMICS_TYPE_CTX.type.get();
        const currComic = comic();
        if (!currComic || !currComic.title) {
            $p.textContent = item.name;
        } else {
            const series = displayable(currComic.title.split('Vol')[0])
                ? currComic.title.split('Vol')[0]
                : '';
            const issue = displayable(currComic.issue) ? `#${currComic.issue}` : '';
            const year = displayable(currComic.releaseDate?.releaseYear)
                ? currComic.releaseDate?.releaseYear
                : '';
            $p.textContent = currType === 'cover' ? `${series} ${issue} (${year})` : item.name;
        }
    };

    return UltraLink({
        href: readerHref,

        children: [
            UltraComponent({
                component: `<p class="${styles.title}"></p>`,
                attributes: {
                    title: item.name,
                },
                onMount: [onTitleChange],
                trigger: [
                    {
                        subscriber: [subsComic, COMICS_TYPE_CTX.type.subscribe],
                        triggerFunction: onTitleChange,
                    },
                ],
            }),
        ],
    });
}
