import styles from './footer.module.css';

const CLIENT_IMPL = 'React';

export function Footer() {
    return (
        <footer className={styles.footer}>
            BetterRack v{__APP_VERSION__} · {CLIENT_IMPL}
        </footer>
    );
}
