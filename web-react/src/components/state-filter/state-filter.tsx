import { CycleButton } from '@/components/cycle-button/cycle-button';
import { useReadTypesContext } from '@/context/ReadTypesContext.hooks';

export function StateFilter() {
    const { type, next } = useReadTypesContext();

    return <CycleButton state={type} onNext={next} />;
}
