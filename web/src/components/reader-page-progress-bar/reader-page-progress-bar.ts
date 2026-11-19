import { UltraComponent } from 'ultra-light-js';
import styles from '../../pages/reader.page.module.css';

export function ReaderPageProgressBar({
    subsCurrentPage,
    subsPages,
    pages,
    currentPage,
}: {
    currentPage: () => number;
    subsCurrentPage: (fn: (value: number) => void) => () => void;
    pages: () => string[];
    subsPages: (fn: (value: string[]) => void) => () => void;
}) {
    const onProgressChange = ($el: HTMLElement) => {
        const total = pages().length || 1;
        const per = (currentPage() / total) * 100;
        $el.style.width = `${per}%`;
    };

    return UltraComponent({
        component: '<div></div>',
        className: [styles.progressTrack],
        children: [
            UltraComponent({
                component: '<div></div>',
                className: [styles.progressFill],
                trigger: [
                    {
                        subscriber: [subsPages, subsCurrentPage],
                        triggerFunction: onProgressChange,
                        defer: true,
                    },
                ],
            }),
        ],
    });
}
