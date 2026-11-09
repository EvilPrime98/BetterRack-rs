import { UltraComponent, ultraState } from 'ultra-light-js';
import styles from './window-controls.module.css';

const MINIMIZE_ICON = `<svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor">
    <path d="M0 5.5H10" />
</svg>`;

const MAXIMIZE_ICON = `<svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor">
    <rect x="0.5" y="0.5" width="9" height="9" />
</svg>`;

const RESTORE_ICON = `<svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor">
    <path d="M2.5 2.5V0.5H9.5V7.5H7.5" />
    <rect x="0.5" y="2.5" width="7" height="7" />
</svg>`;

const CLOSE_ICON = `<svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor">
    <path d="M0.5 0.5L9.5 9.5M9.5 0.5L0.5 9.5" />
</svg>`;

export function WindowControls() {
    const isEnabled = !!window.desktop?.minimizeWindow && window.versions?.platform !== 'darwin';
    if (!isEnabled) return null;

    const [isMaximized, setIsMaximized, subsIsMaximized] = ultraState(false);

    const onMaximizedChange = ($button: HTMLElement) => {
        $button.setAttribute('aria-label', isMaximized() ? 'Restore' : 'Maximize');
        $button.innerHTML = isMaximized() ? RESTORE_ICON : MAXIMIZE_ICON;
    };

    return UltraComponent({
        component: '<div></div>',

        className: [styles.controls],

        onMount: [
            () => {
                window.desktop?.isWindowMaximized().then(setIsMaximized);
                return window.desktop?.onMaximizedChange(setIsMaximized);
            },
        ],

        children: [
            UltraComponent({
                component: `<button type="button">${MINIMIZE_ICON}</button>`,
                className: [styles.button],
                attributes: { 'aria-label': 'Minimize' },
                eventHandler: { click: () => window.desktop?.minimizeWindow() },
            }),

            UltraComponent({
                component: `<button type="button">${MAXIMIZE_ICON}</button>`,
                className: [styles.button],
                attributes: { 'aria-label': 'Maximize' },
                eventHandler: { click: () => window.desktop?.toggleMaximizeWindow() },
                trigger: [{ subscriber: subsIsMaximized, triggerFunction: onMaximizedChange }],
            }),

            UltraComponent({
                component: `<button type="button">${CLOSE_ICON}</button>`,
                className: [styles.button, styles.close],
                attributes: { 'aria-label': 'Close' },
                eventHandler: { click: () => window.desktop?.closeWindow() },
            }),
        ],
    });
}
