import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import styles from './new-folder-modal.module.css';
import { BRButton } from '@/components/br-button/br-button';
import { useNewFolderModalContext } from '@/context/NewFolderModalContext.hooks';
import { useModalDialog } from '@/hooks/useModalDialog';

export function NewFolderModal() {
    const { isVisible, closeNewFolderModal, submitNewFolder } = useNewFolderModalContext();

    const [text, setText] = useState('');
    const textRef = useRef(text);
    const inputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        textRef.current = text;
    }, [text]);

    const cancel = () => closeNewFolderModal();
    const submit = () => submitNewFolder(textRef.current);

    // Reads textRef.current (kept fresh above) rather than `text`, so it never closes over a stale value.
    useEffect(() => {
        if (!isVisible) return;
        const onKeydown = (e: KeyboardEvent) => {
            if (e.key === 'Enter') submitNewFolder(textRef.current);
        };
        document.addEventListener('keydown', onKeydown);
        return () => document.removeEventListener('keydown', onKeydown);
    }, [isVisible, closeNewFolderModal, submitNewFolder]);

    useLayoutEffect(() => {
        if (!isVisible || !inputRef.current) return;
        inputRef.current.value = '';
        setText('');
        inputRef.current.focus();
    }, [isVisible]);

    const dialogProps = useModalDialog(isVisible, closeNewFolderModal);

    return (
        <dialog className={styles.overlay} {...dialogProps} aria-label="Create folder">
            <button
                type="button"
                className={styles.backdrop}
                tabIndex={-1}
                aria-label="Close dialog"
                onClick={cancel}
            />

            <div className={styles.modal}>
                <p className={styles.title}>New folder</p>

                <input
                    ref={inputRef}
                    type="text"
                    className={styles.field}
                    placeholder="Folder name"
                    aria-label="Folder name"
                    onChange={(e) => setText(e.currentTarget.value)}
                />

                <div className={styles.actions}>
                    <BRButton text="Cancel" variant="secondary" onClick={cancel} />
                    <BRButton text="Create" variant="classic" onClick={submit} />
                </div>
            </div>
        </dialog>
    );
}
