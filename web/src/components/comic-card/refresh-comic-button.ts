import { UltraComponent } from 'ultra-light-js';
import styles from './refresh-comic-button.module.css';
import { RefreshIcon } from '@/icons/refresh-icon';
import { retryThumbnail, reidentifyFile } from '@/services/library.service';
import { COMIC_IDENT_CTX } from '@/context/identifer-modal.context';

export function RefreshComicButton({ uid, onRefreshed }: { uid: string; onRefreshed: () => void }) {
    const onClick = (e: Event) => {
        e.stopPropagation();
        e.preventDefault();
        const $button = e.currentTarget as HTMLElement;
        if ($button.getAttribute('aria-busy') === 'true') return;
        $button.setAttribute('aria-busy', 'true');
        Promise.allSettled([retryThumbnail(uid), reidentifyFile(uid)])
            .then(([, identifyResult]) => {
                if (identifyResult.status !== 'fulfilled') return;
                const { identified, comic, metaSource } = identifyResult.value;
                if (identified && comic && metaSource) {
                    COMIC_IDENT_CTX.lastIdentified.set({ uid, comic, metaSource });
                } else {
                    COMIC_IDENT_CTX.lastUnidentified.set({ uid });
                }
            })
            .finally(() => {
                $button.setAttribute('aria-busy', 'false');
                onRefreshed();
            });
    };

    return UltraComponent({
        component: '<span></span>',
        className: [styles.refreshComicButton],
        attributes: {
            type: 'button',
            'aria-label': 'Regenerate the cover thumbnail and re-identify this comic',
            'aria-busy': 'false',
        },
        eventHandler: {
            click: onClick,
        },
        children: [RefreshIcon({ size: 16 })],
    });
}
