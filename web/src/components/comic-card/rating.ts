import { UltraComponent, ultraState } from 'ultra-light-js';
import styles from './comic-card.module.css';
import { StarComponent } from '../star-component/star-component';
import { COMIC_CACHE_CONTEXT } from '../../context/comic-cache.context';

export function ComicRating({ uid }: { uid: string }) {
    const [rating, setRating, subsRating] = ultraState(
        COMIC_CACHE_CONTEXT.getCacheById(uid)?.rating || 0,
    );

    COMIC_CACHE_CONTEXT.subscribeById(uid, (entry) => {
        setRating(entry?.rating || 0);
    });

    const changeRating = (newRating: number) => {
        setRating(newRating);
        COMIC_CACHE_CONTEXT.setCacheById(uid, { rating: newRating });
    };

    let $container: HTMLElement | null = null;

    const applyFill = (value: number) => {
        const stars = $container?.children;
        if (!stars) return;
        for (let i = 0; i < stars.length; i++) {
            const $fillRect = stars[i].querySelector('.fillRect');
            const fraction = Math.max(0, Math.min(1, value - i));
            $fillRect?.setAttribute('width', String(16 * fraction));
        }
    };

    const onRatingChange = ($div: HTMLElement) => {
        $container = $div;
        const stars = [];
        for (let i = 0; i < 5; i++) {
            stars.push(
                StarComponent({
                    weight: i + 1,
                    changeRating,
                    onHover: applyFill,
                    initialFill: Math.max(0, Math.min(1, rating() - i)),
                }),
            );
        }
        $div.replaceChildren(...stars);
    };

    return UltraComponent({
        component: '<div></div>',
        className: [styles.rating],
        onMount: [onRatingChange],
        eventHandler: {
            mouseleave: () => applyFill(rating()),
        },
        trigger: [
            {
                subscriber: subsRating,
                triggerFunction: onRatingChange,
            },
        ],
    });
}
