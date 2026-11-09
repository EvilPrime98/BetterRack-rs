import type { WikiComic } from 'better-wiki';
import { UltraComponent } from 'ultra-light-js';
import styles from './comic-identifier.module.css';
import { ImageGen } from '../image-generic/image-generic';
import { COMIC_IDENT_CTX } from '../../context/identifer-modal.context';
import { updateComicIdentity } from '../../services/comic-data.service';

export function SuggestionCard({ comic }: { comic: WikiComic }) {
    const meta = [comic.volume, comic.issue ? `#${comic.issue}` : null].filter(Boolean).join(' · ');

    const onClick = () => {
        const uid = COMIC_IDENT_CTX.itemUid.get();
        updateComicIdentity(uid, {
            prefId: comic.pageId,
            sourceWiki: comic.sourceWiki,
            identified: true,
            comic,
        }).catch(console.error);
        COMIC_IDENT_CTX.lastIdentified.set({ uid, comic, metaSource: 'wiki' });
        COMIC_IDENT_CTX.isVisible.set(false);
    };

    return UltraComponent({
        component: '<article></article>',

        className: [styles.suggestionCard],

        eventHandler: {
            click: onClick,
        },

        children: [
            UltraComponent({
                component: '<div></div>',
                className: [styles.suggestionCover],
                children: [
                    ImageGen({
                        attributes: {
                            src: comic.cover,
                        },
                    }),
                ],
            }),

            UltraComponent({
                component: '<div></div>',
                className: [styles.suggestionInfo],
                children: [
                    `<p class="${styles.suggestionTitle}">${comic.title}</p>`,
                    `<p class="${styles.suggestionMeta}">${meta}</p>`,
                ],
            }),
        ],
    });
}
