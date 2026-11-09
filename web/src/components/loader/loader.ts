import { UltraActivity } from 'ultra-light-js';
import styles from './loader.module.css';

export function Loader({
    mode,
    label,
    size = 28,
    className,
}: {
    mode: {
        state: () => boolean;
        subscriber: ((fn: () => void) => () => void) | ((fn: () => void) => () => void)[];
    };
    label?: string;
    size?: number;
    className?: string;
}) {
    const borderWidth = Math.max(2, Math.round(size * 0.11));

    return UltraActivity({
        mode,

        component: '<div></div>',

        className: [styles.loader, ...(className ? [className] : [])],

        children: [
            `<div class="${styles.spinner}" style="width:${size}px;height:${size}px;border-width:${borderWidth}px"></div>`,
            ...(label ? [`<p class="${styles.label}">${label}</p>`] : []),
        ],
    });
}
