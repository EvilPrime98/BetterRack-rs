import styles from './app-loader.module.css';

export function AppLoader({
    visible = true,
    message = 'Loading your library…',
}: {
    visible?: boolean;
    message?: string;
} = {}) {
    return (
        <div className={styles.overlay} style={{ display: visible ? undefined : 'none' }}>
            <img src="/favicon.svg" alt="BetterRack" className={styles.logo} />
            <p className={styles.brand}>
                Better<span>Rack</span>
            </p>
            <div className={styles.spinner} />
            <p className={styles.hint}>{message}</p>
        </div>
    );
}
