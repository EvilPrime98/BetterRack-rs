import { UltraComponent } from 'ultra-light-js';
import { LIBRARY_CONTEXT } from '../../context/library.context';
import styles from './sidebar.module.css';

export function RefreshLibraryButton() {
    function refreshLibrary() {
        LIBRARY_CONTEXT.refreshLibraryWithPrompt();
    }

    function onRefreshingChange($button: HTMLElement) {
        const progress = LIBRARY_CONTEXT.identifyProgress.get();
        const isBusy = LIBRARY_CONTEXT.queryClient.get().isFetching() || progress !== null;
        $button.classList.toggle(styles.spinning, isBusy);
        ($button as HTMLButtonElement).disabled = isBusy;
        const $label = $button.lastElementChild;
        if ($label) {
            $label.textContent = progress
                ? `Identifying${progress.total ? ` ${progress.done}/${progress.total}` : '…'}`
                : 'Refresh Libraries';
        }
    }

    return UltraComponent({
        onMount: [onRefreshingChange],
        component: '<button></button>',
        className: [styles.refreshButton],
        attributes: { 'aria-label': 'Refresh library' },
        eventHandler: { click: refreshLibrary },
        children: [
            `<span class="${styles.refreshSpinner}"></span>`,
            `<span>Refresh Libraries</span>`,
        ],
        trigger: [
            {
                subscriber: [
                    LIBRARY_CONTEXT.queryClient.get().subscribeToFetching,
                    LIBRARY_CONTEXT.identifyProgress.subscribe,
                ],
                triggerFunction: onRefreshingChange,
            },
        ],
    });
}
