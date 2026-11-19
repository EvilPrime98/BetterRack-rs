import { CloseIcon } from '@/icons/close.icon';
import { ArrowLeftIcon } from '@/icons/arrow-left.icon';
import styles from './sidebar.module.css';
import { useSidebarStore } from '@/stores/sidebar.store';
import { useIsDesktop } from '@/hooks/useIsDesktop';

export function SidebarCloseButton() {
    const isDesktop = useIsDesktop();
    const setIsExpanded = useSidebarStore((s) => s.setIsExpanded);
    const setIsCollapsed = useSidebarStore((s) => s.setIsCollapsed);

    function handleClick() {
        // Above 800px the sidebar is permanent, so "close" collapses it to zero
        // width and lets the page content grow. Below that it dismisses the overlay.
        if (isDesktop) {
            setIsCollapsed(true);
        } else {
            setIsExpanded(false);
        }
    }

    return (
        <button
            type="button"
            className={styles.button}
            aria-label={isDesktop ? 'Collapse sidebar' : 'Close sidebar'}
            onClick={handleClick}
        >
            {isDesktop ? <ArrowLeftIcon size={20} /> : <CloseIcon size={20} />}
        </button>
    );
}
