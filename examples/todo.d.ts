/* tslint:disable */
/* eslint-disable */

export class State {
    free(): void;
    [Symbol.dispose](): void;
    get_items(): number;
    get_signal(id: number): number;
    invoke_add_item(): void;
    invoke_remove_item(): void;
    constructor();
    poll_dispatch(): any;
    set_items(value: number): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_state_free: (a: number, b: number) => void;
    readonly state_get_items: (a: number) => number;
    readonly state_get_signal: (a: number, b: number) => number;
    readonly state_invoke_add_item: (a: number) => void;
    readonly state_invoke_remove_item: (a: number) => void;
    readonly state_new: () => number;
    readonly state_poll_dispatch: (a: number) => any;
    readonly state_set_items: (a: number, b: number) => void;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
