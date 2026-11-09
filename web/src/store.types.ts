import type { TJobStatus } from './services/store.service';

export interface IStorePost {
    id?: number;
    thumbnailUrl?: string;
    title: string;
    link: string;
    uploadDate?: string;
}

export interface IStoreLink {
    uuid: string;
    title: string;
}

export type TStoreStrat = 'all' | 'single' | 'multiple';

export type TStoreProgressEvent =
    | { type: 'preparing'; title: string }
    | {
          type: 'retrying';
          title: string;
          status?: number;
          reason?: 'network' | 'http';
          delaySec: number;
      }
    | { type: 'progress'; title: string; percent: number; receivedMB: string; totalMB: string }
    | { type: 'extracting'; title: string; done: number; total: number }
    | { type: 'done'; filename: string }
    | { type: 'error'; message: string };

export type TCardState =
    { status: 'idle' } | { status: 'links-loading' } | { status: 'error'; message: string };

export const STRAT = 'all';

export type TJobStates = Map<string, TJobStatus['state']>;
