import { useEffect } from 'react';
import type { RefObject } from 'react';
import type { IShortcut } from '@/lib/shortcuts';
import { applyMove, readerMoveFor } from '@/lib/reader-keys';
import type { TReaderMove } from '@/lib/reader-keys';
import { SHORTCUTS } from '@/services/shortcuts.service';

export function useReaderKeys({
    pageRef,
}: {
    pageRef: RefObject<HTMLElement | null>;
}) {
    useEffect(() => {
        const moveBy = (e: KeyboardEvent) => {
            const move = readerMoveFor(e.key, e.shiftKey);
            const $scroller = pageRef.current;
            if (!move || !$scroller) return;
            if ($scroller.dataset.mode === 'horizontal') {
                const pageMove: TReaderMove =
                    move.type === 'to'
                        ? move
                        : { type: 'by', unit: 'viewport', amount: Math.sign(move.amount) };
                $scroller.scrollLeft = applyMove(pageMove, {
                    offset: $scroller.scrollLeft,
                    viewport: $scroller.clientWidth,
                    max: $scroller.scrollWidth - $scroller.clientWidth,
                });
                return;
            }
            $scroller.scrollTop = applyMove(move, {
                offset: $scroller.scrollTop,
                viewport: $scroller.clientHeight,
                max: $scroller.scrollHeight - $scroller.clientHeight,
            });
        };

        const shortcuts: IShortcut[] = [
            { key: 'PageDown', handler: moveBy },
            { key: 'PageUp', handler: moveBy },
            { key: ' ', handler: moveBy },
            { key: ' ', shift: true, handler: moveBy },
            { key: 'ArrowDown', handler: moveBy },
            { key: 'ArrowUp', handler: moveBy },
            { key: 'Home', handler: moveBy },
            { key: 'End', handler: moveBy },
        ];
        return SHORTCUTS.register(shortcuts);
    }, [pageRef]);
}
