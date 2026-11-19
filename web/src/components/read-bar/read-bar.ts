import { UltraComponent } from 'ultra-light-js';
import styles from './read-bard.module.css';

export function ReadBar({
    getReadPercentage,
    subsReadPercentage,
}: {
    getReadPercentage: () => number;
    subsReadPercentage: (fn: (value: unknown) => void) => () => void;
}) {
    const onReadPercentageChange = ($fill: HTMLElement) => {
        const per = getReadPercentage();
        $fill.style.width = `${per}%`;
        $fill.style.backgroundColor = per === 100 ? '#13a629' : '#34c3d1';
    };

    return UltraComponent({
        component: '<div></div>',

        className: [styles.readBar],

        children: [
            UltraComponent({
                component: '<div></div>',
                className: [styles.fillIn],
                onMount: [onReadPercentageChange],
                trigger: [
                    {
                        subscriber: subsReadPercentage,
                        triggerFunction: onReadPercentageChange,
                    },
                ],
            }),
        ],
    });
}
