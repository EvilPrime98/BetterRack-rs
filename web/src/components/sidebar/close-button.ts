import { UltraComponent } from 'ultra-light-js';
import { CloseIcon } from '../../icons/close.icon';
import { ArrowLeftIcon } from '../../icons/arrow-left.icon';
import styles from './sidebar.module.css';
import { SIDEBAR_CONTEXT } from '../../context/sidebar.context';
import { VIEWPORT_CONTEXT } from '../../context/viewport.context';

export function SidebarCloseButton() {
    function handleClick() {
        if (VIEWPORT_CONTEXT.isDesktop.get()) {
            SIDEBAR_CONTEXT.isCollapsed.set(true);
        } else {
            SIDEBAR_CONTEXT.isExpanded.set(false);
        }
    }

    function onModeChange($button: HTMLElement) {
        const isDesktop = VIEWPORT_CONTEXT.isDesktop.get();
        $button.innerHTML = isDesktop ? ArrowLeftIcon({ size: 20 }) : CloseIcon({ size: 20 });
        $button.setAttribute('aria-label', isDesktop ? 'Collapse sidebar' : 'Close sidebar');
    }

    return UltraComponent({
        component: '<div></div>',
        className: [styles.button],
        attributes: { role: 'button' },
        eventHandler: { click: handleClick },
        onMount: [onModeChange],
        trigger: [
            {
                subscriber: VIEWPORT_CONTEXT.isDesktop.subscribe,
                triggerFunction: onModeChange,
            },
        ],
    });
}
