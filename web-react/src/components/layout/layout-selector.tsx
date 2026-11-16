import { CycleButton } from '@/components/cycle-button/cycle-button';
import { useComicsTypeStore } from '@/stores/comicsTypes.store';

export function LayoutSelector() {
    const type = useComicsTypeStore((s) => s.type);
    const next = useComicsTypeStore((s) => s.next);

    return <CycleButton state={type} onNext={next} />;
}
