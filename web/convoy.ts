export const ACTIONS = [
  { id: 1, label: 'Hold convoy · check strap' },
  { id: 2, label: 'Ask Mara what she saw' },
  { id: 3, label: 'Release convoy · take receipt' },
  { id: 4, label: 'Give receipt to Sela · priority pouch' },
  { id: 5, label: 'Visit / return to Ivo at depot' },
  { id: 6, label: 'Advance fictional time · 1 hour' },
] as const;

export type Action = 0 | (typeof ACTIONS)[number]['id'];
type Abi = {
  dw_version(): number;
  dw_init(): number;
  dw_act(action: number): number;
  dw_time(): number;
  dw_enabled(action: number): number;
  dw_len(field: number): number;
  dw_byte(field: number, index: number): number;
};

const ARITY = { dw_version: 0, dw_init: 0, dw_act: 1, dw_time: 0, dw_enabled: 1, dw_len: 1, dw_byte: 2 } as const;
const MAX_TEXT_BYTES = 1024 * 1024;

function uint(value: number, maximum: number, name: string): number {
  if (!Number.isInteger(value) || value < 0 || value > maximum) {
    throw new Error(`Invalid Wasm ${name}: ${value}.`);
  }
  return value;
}

export class Convoy {
  #abi: Abi;

  constructor(instance: WebAssembly.Instance) {
    for (const [name, arity] of Object.entries(ARITY)) {
      const value = instance.exports[name];
      if (typeof value !== 'function' || value.length !== arity) {
        throw new Error(`Missing or incompatible Wasm export: ${name}.`);
      }
    }
    this.#abi = instance.exports as Abi;
    if (this.#abi.dw_version() !== 1) throw new Error('Unsupported Wasm ABI version; expected 1.');
    if (this.#abi.dw_init() !== 0) throw new Error('Wasm initialization failed.');
  }

  act(action: Action): number {
    uint(action, 6, 'action');
    return uint(this.#abi.dw_act(action), 2, 'action status');
  }

  #text(field: 0 | 1 | 2): string {
    const length = uint(this.#abi.dw_len(field), MAX_TEXT_BYTES, 'text length');
    const bytes = new Uint8Array(length);
    for (let index = 0; index < length; index += 1) {
      bytes[index] = uint(this.#abi.dw_byte(field, index), 255, 'text byte');
    }
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  }

  snapshot() {
    return {
      time: uint(this.#abi.dw_time(), 0xffff_ffff, 'time'),
      dialogue: this.#text(0),
      journal: this.#text(1).split('\n').filter(Boolean),
      choices: ACTIONS.map(action => ({ ...action, enabled: uint(this.#abi.dw_enabled(action.id), 1, 'choice status') === 1 })),
    };
  }

  developerTrace(): string {
    return this.#text(2);
  }
}

export async function loadConvoy(url: URL | string): Promise<Convoy> {
  let response: Response;
  try {
    response = await fetch(url);
  } catch (error) {
    throw new Error('Could not fetch convoy.wasm. Check the connection and reload.', { cause: error });
  }
  if (!response.ok) throw new Error(`Could not fetch convoy.wasm: HTTP ${response.status}.`);
  try {
    const { instance } = await WebAssembly.instantiate(await response.arrayBuffer(), {});
    return new Convoy(instance);
  } catch (error) {
    throw new Error(`Could not start convoy.wasm: ${error instanceof Error ? error.message : String(error)}`, { cause: error });
  }
}
