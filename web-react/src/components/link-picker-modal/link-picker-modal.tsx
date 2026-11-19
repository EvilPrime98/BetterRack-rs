import styles from './link-picker-modal.module.css';
import { useLinkPickerModalContext } from '@/context/LinkPickerModalContext.hooks';
import { useModalDialog } from '@/hooks/useModalDialog';

export function LinkPickerModal() {
    const { isVisible, comicTitle, links, closeLinkPickerModal, pickLink } =
        useLinkPickerModalContext();

    const cancel = () => closeLinkPickerModal();

    const dialogProps = useModalDialog(isVisible, cancel);

    return (
        <dialog className={styles.overlay} {...dialogProps} aria-label="Choose a download link">
            <button
                type="button"
                className={styles.backdrop}
                tabIndex={-1}
                aria-label="Close dialog"
                onClick={cancel}
            />

            <div className={styles.modal}>
                <div className={styles.header}>
                    <p className={styles.title}>Choose a link</p>
                    {comicTitle && (
                        <p className={styles.subtitle} title={comicTitle}>
                            {comicTitle}
                        </p>
                    )}
                </div>

                <ul className={styles.list} role="listbox">
                    {links.length === 0 && <li className={styles.empty}>No links available.</li>}

                    {links.map((link, i) => (
                        <li
                            key={link.uuid}
                            className={styles.item}
                            role="option"
                            aria-selected={false}
                            tabIndex={0}
                            onClick={() => pickLink(link)}
                            onKeyDown={(e) => {
                                if (e.key !== 'Enter' && e.key !== ' ') return;
                                e.preventDefault();
                                pickLink(link);
                            }}
                        >
                            <span className={styles.index}>{i + 1}</span>
                            <span title={link.title}>{link.title}</span>
                        </li>
                    ))}
                </ul>
            </div>
        </dialog>
    );
}
