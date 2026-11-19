import styles from './confirm-modal.module.css';
import { BRButton } from '@/components/br-button/br-button';
import { BRCheckbox } from '@/components/br-checkbox/br-checkbox';
import { useConfirmModalStore } from '@/stores/confirmModal.store';
import { useModalDialog } from '@/hooks/useModalDialog';

export function ConfirmModal() {
    const isVisible = useConfirmModalStore((s) => s.isVisible);
    const title = useConfirmModalStore((s) => s.title);
    const message = useConfirmModalStore((s) => s.message);
    const confirmLabel = useConfirmModalStore((s) => s.confirmLabel);
    const cancelLabel = useConfirmModalStore((s) => s.cancelLabel);
    const hasDontAskAgain = useConfirmModalStore((s) => s.hasDontAskAgain);
    const dontAskAgain = useConfirmModalStore((s) => s.dontAskAgain);

    const dismiss = () => useConfirmModalStore.getState().resolveConfirmDialog(null);
    const cancel = () => useConfirmModalStore.getState().resolveConfirmDialog(false);
    const confirm = () => useConfirmModalStore.getState().resolveConfirmDialog(true);

    const dialogProps = useModalDialog(isVisible, dismiss);

    return (
        <dialog className={styles.overlay} {...dialogProps} aria-label="Confirm action">
            <button
                type="button"
                className={styles.backdrop}
                tabIndex={-1}
                aria-label="Close dialog"
                onClick={dismiss}
            />

            <div className={styles.modal}>
                <p className={styles.title}>{title}</p>

                <p className={styles.message}>{message}</p>

                {hasDontAskAgain && (
                    <BRCheckbox
                        text="Don't ask again"
                        checked={dontAskAgain}
                        onChange={(e) =>
                            useConfirmModalStore.getState().setDontAskAgain(e.currentTarget.checked)
                        }
                    />
                )}

                <div className={styles.actions}>
                    <BRButton text={cancelLabel} variant="secondary" onClick={cancel} />

                    <BRButton text={confirmLabel} variant="classic" onClick={confirm} />
                </div>
            </div>
        </dialog>
    );
}
