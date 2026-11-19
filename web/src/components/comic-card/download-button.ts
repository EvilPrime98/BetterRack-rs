import { UltraComponent } from 'ultra-light-js';
import styles from './download-button.module.css';
import { DownloadIcon } from '@/icons/download.icon';
import { downloadComic } from '@/services/download.service';
import { toast } from '@/services/toast.service';

export function DownloadComicButton({ uid, name }: { uid: string; name: string }) {
    const onClick = (e: Event) => {
        e.stopPropagation();
        e.preventDefault();
        const $button = e.currentTarget as HTMLElement;
        if ($button.getAttribute('aria-busy') === 'true') return;
        $button.setAttribute('aria-busy', 'true');
        downloadComic(uid, name)
            .then((savedPath) => {
                if (savedPath) toast.success(`Saved to ${savedPath}`);
            })
            .catch((e) => {
                toast.error(
                    e instanceof Error ? e.message : 'There was an error downloading the comic.',
                );
            })
            .finally(() => $button.setAttribute('aria-busy', 'false'));
    };

    return UltraComponent({
        component: '<span></span>',
        className: [styles.downloadComicButton],
        attributes: {
            type: 'button',
            'aria-label': 'Download this comic to your device',
            title: 'Download this comic to your device',
            'aria-busy': 'false',
        },
        eventHandler: {
            click: onClick,
        },
        children: [DownloadIcon({ size: 16 })],
    });
}
