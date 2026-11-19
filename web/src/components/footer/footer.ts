import { UltraComponent } from 'ultra-light-js';
import styles from './footer.module.css';

const CLIENT_IMPL = 'ultra-light-js';

export function Footer() {
    return UltraComponent({
        component: '<footer></footer>',
        className: [styles.footer],
        children: [`<span>BetterRack v${__APP_VERSION__} · ${CLIENT_IMPL}</span>`],
    });
}
