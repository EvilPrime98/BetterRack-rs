import styles from './loader.module.css';

export function Loader({
    visible,
    label,
    size = 28,
    className,
}: {
    visible: boolean;
    label?: string;
    size?: number;
    className?: string;
}) {
    return (
        <div
            className={[styles.loader, className].filter(Boolean).join(' ')}
            style={{ display: visible ? undefined : 'none' }}
        >
            <div
                className={styles.spinner}
                style={{
                    width: size,
                    height: size,
                    borderWidth: Math.max(2, Math.round(size * 0.11)),
                }}
            />
            {label && <p className={styles.label}>{label}</p>}
        </div>
    );
}
