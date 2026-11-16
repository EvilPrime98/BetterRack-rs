import { useEffect, useRef } from 'react';
import type { RefObject } from 'react';
import type { IShortcut } from '@/lib/shortcuts';
import { applyMove, readerMoveFor } from '@/lib/reader-keys';
import { SHORTCUTS } from '@/services/shortcuts.service';

export function useReaderKeys({
    pageRef,
    isHorizontal,
    pageCount,
    currentPage,
    goToPage,
}: {
    pageRef: RefObject<HTMLElement | null>;
    isHorizontal: boolean;
    pageCount: number;
    currentPage: number;
    goToPage: (page: number) => void;
}) {
    const latest = useRef({ isHorizontal, pageCount, currentPage, goToPage });
    useEffect(() => {
        latest.current = { isHorizontal, pageCount, currentPage, goToPage };
    });

    useEffect(() => {
        const moveBy = (e: KeyboardEvent) => {
            const move = readerMoveFor(e.key, e.shiftKey);
            const $scroller = pageRef.current;
            if (!move || !$scroller) return;
            const state = latest.current;
            if (state.isHorizontal) {
                const last = state.pageCount;
                const target =
                    move.type === 'to'
                        ? move.edge === 'start'
                            ? 1
                            : last
                        : state.currentPage + (move.amount > 0 ? 1 : -1);
                state.goToPage(Math.min(last, Math.max(1, target)));
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
