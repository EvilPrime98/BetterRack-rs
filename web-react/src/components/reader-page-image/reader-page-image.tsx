import { memo, useLayoutEffect, useRef, useState } from 'react';
import styles from '../../pages/reader.page.module.css';
import { ImageGen } from '../image-generic/image-generic';
import { API_URL } from '../../services/library.service';
import { withAuthQuery } from '../../services/server-config.service';

const MAX_SNAP_OVERFLOW = 1.2;

export const ImageElement = memo(function ImageElement({
    uid,
    ind,
    index,
    total,
    active,
    onRatio,
}: {
    uid: string;
    ind: number;
    index: number;
    total: number;
    active: boolean;
    onRatio: (ratio: number) => void;
}) {
    const [isLoaded, setIsLoaded] = useState(false);
    const [ratio, setRatio] = useState<number | null>(null);
    const [snap, setSnap] = useState(false);
    const figureRef = useRef<HTMLElement>(null);

    useLayoutEffect(() => {
        const $figure = figureRef.current;
        if (!$figure || !ratio) return;
        const measure = () => {
            const { clientWidth, clientHeight } = $figure;
            const overflow = clientWidth > 0 ? (clientHeight * ratio) / clientWidth : 0;
            setSnap(overflow > 1 && overflow <= MAX_SNAP_OVERFLOW);
        };
        measure();
        const observer = new ResizeObserver(measure);
        observer.observe($figure);
        return () => observer.disconnect();
    }, [ratio]);

    if (!active && isLoaded) setIsLoaded(false);

    const className = [styles.pageWrapper, isLoaded ? styles.loaded : '', active ? '' : styles.idle]
        .filter(Boolean)
        .join(' ');

    return (
        <figure
            ref={figureRef}
            className={className}
            data-snap={snap ? '' : undefined}
            style={
                ratio
                    ? ({ aspectRatio: ratio, '--img-ratio': ratio } as React.CSSProperties)
                    : undefined
            }
        >
            {active ? (
                <ImageGen
                    src={withAuthQuery(`${API_URL}/read/${uid}/pages/${ind}`)}
                    alt={`${uid} — page ${index} of ${total}`}
                    loading="eager"
                    onLoad={(e) => {
                        const { naturalWidth, naturalHeight } = e.currentTarget;
                        if (naturalWidth > 0 && naturalHeight > 0) {
                            const measured = naturalWidth / naturalHeight;
                            setRatio(measured);
                            onRatio(measured);
                        }
                        setIsLoaded(true);
                    }}
                />
            ) : null}
        </figure>
    );
});
