import { UltraActivity, UltraComponent } from 'ultra-light-js';
import styles from './app-loader.module.css';

export function AppLoader({
    mode,
    message = 'Loading your library&hellip;',
}: {
    message?: string;
    mode?: {
        state: () => boolean;
        subscriber: ((fn: () => void) => () => void) | ((fn: () => void) => () => void)[];
    };
} = {}) {
    if (!mode) {
        return UltraComponent({
            component: '<div></div>',
            className: [styles.overlay],
            children: [
                `<img src="/favicon.svg" alt="BetterRack" class="${styles.logo}" />`,
                `<p class="${styles.brand}">Better<span>Rack</span></p>`,
                `<div class="${styles.spinner}"></div>`,
                `<p class="${styles.hint}">${message}</p>`,
            ],
        });
    }

    return UltraActivity({
        mode,
        component: '<div></div>',
        className: [styles.overlay],
        children: [
            `<img src="/favicon.svg" alt="BetterRack" class="${styles.logo}" />`,
            `<p class="${styles.brand}">Better<span>Rack</span></p>`,
            `<div class="${styles.spinner}"></div>`,
            `<p class="${styles.hint}">${message}</p>`,
        ],
    });
}
