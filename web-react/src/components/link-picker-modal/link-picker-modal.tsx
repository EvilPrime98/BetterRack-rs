import { useEffect } from 'react';
import styles from './link-picker-modal.module.css';
import { useLinkPickerModalContext } from '@/context/LinkPickerModalContext.hooks';

export function LinkPickerModal() {
    const { isVisible, comicTitle, links, closeLinkPickerModal, pickLink } =
        useLinkPickerModalContext();

    const cancel = () => closeLinkPickerModal();

    useEffect(() => {
        const onKeydown = (e: KeyboardEvent) => {
            if (e.key === 'Escape') cancel();
        };
        document.addEventListener('keydown', onKeydown);
        return () => document.removeEventListener('keydown', onKeydown);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    return (
        <div className={styles.overlay} style={{ display: isVisible ? undefined : 'none' }}>
            <button
                type="button"
                className={styles.backdrop}
                aria-label="Close dialog"
                onClick={cancel}
            />

            <div
                role="dialog"
                aria-modal="true"
                aria-label="Choose a download link"
                className={styles.modal}
            >
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
        </div>
    );
}
