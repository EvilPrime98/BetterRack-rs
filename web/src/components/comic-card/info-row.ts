import { UltraComponent } from 'ultra-light-js';
import styles from './comic-card.module.css';
import type { WikiComic } from 'better-wiki';

const notDisplayable = ['', 'undefined'];

export function InfoRow({
    label,
    subsComic,
    onValueChange,
}: {
    label: string;
    subsComic: (fn: (value: WikiComic | null) => void) => () => void;
    onValueChange: ($span: HTMLElement) => void;
}) {
    const updateRow = ($row: HTMLElement) => {
        const $value = $row.querySelector(`.${styles.infoValue}`) as HTMLElement;
        onValueChange($value);
        $row.style.display = notDisplayable.includes($value.textContent || '') ? 'none' : '';
    };

    return UltraComponent({
        component: '<div></div>',
        className: [styles.infoRow],
        onMount: [updateRow],
        trigger: [
            {
                subscriber: subsComic,
                triggerFunction: updateRow,
            },
        ],
        children: [
            `<span class="${styles.infoLabel}">${label}</span>`,
            `<span class="${styles.infoValue}"></span>`,
        ],
    });
}
