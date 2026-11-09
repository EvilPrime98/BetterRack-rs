import { UltraComponent } from 'ultra-light-js';
import { ImageGen } from '../image-generic/image-generic';
import styles from '../comic-card/comic-card.module.css';

export function FolderStackCard({ cover }: { cover: string; isRead: boolean }) {
    const onImageLoad = (e: Event) => {
        const $img = e.currentTarget as HTMLImageElement;
        $img.parentElement?.classList.add(styles.loaded);
    };

    return UltraComponent({
        component: '<article></article>',

        className: [
            styles.comicCard,
            styles.stackLayer,
        ],

        styles: {
            padding: '0',
        },

        children: [
            UltraComponent({
                component: '<div></div>',

                className: [styles.cover],

                children: [
                    `<div class="${styles.board}"></div>`,

                    UltraComponent({
                        component: '<a></a>',

                        children: [
                            ImageGen({
                                eventHandler: {
                                    load: onImageLoad,
                                },
                                attributes: {
                                    src: cover,
                                },
                            }),
                        ],
                    }),

                    `<div class="${styles.bagOverlay}"></div>`,
                ],
            }),
        ],
    });
}
