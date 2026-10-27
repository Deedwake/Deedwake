import { Convoy } from './convoy.ts';
import { CapacityError, ConfiguredConvoy } from './configured.ts';
import type { Action, ConvoyCondition } from './convoy.ts';

const MAGIC = 'deedwake-convoy-checkpoint';
const MAX_ACTIONS = 4096;
const MAX_BYTES = 64 * 1024;
const KEYS = ['magic', 'schema', 'exactWasmSHA256', 'condition', 'actions'];
const CONFIGURED_KEYS = ['magic', 'schema', 'exactWasmSHA256', 'profileId', 'actions'];

function admitConfiguredInput(convoy: ConfiguredConvoy, count: number, action: Action, freshVisit: boolean): void {
  if (action === 0) return;
  if (count >= MAX_ACTIONS) throw new CapacityError('checkpoint_commands');
  if (action !== 2 && !(action === 5 && !freshVisit)) return;
  const view = convoy.snapshot();
  const enabled = (id: number) => view.choices.some(choice => choice.id === id && choice.enabled);
  const oneOffs = enabled(1) ? 3 : enabled(3) ? 2 : enabled(4) ? 1 : 0;
  // Reserve a wait+visit per remaining hour, each one-off+visit, and a fresh visit.
  // These are public clock/choice facts; no privileged simulation data is read.
  const reserve = 2 * (1000 - view.time) + 2 * oneOffs + Number(freshVisit);
  if (count + 1 + reserve > MAX_ACTIONS) throw new CapacityError('checkpoint_commands');
}

function visitAfter(action: Action, previous: boolean): boolean {
  return action === 5 ? false : [0, 1, 3, 4, 6].includes(action) ? true : previous;
}

// JSON.parse discards duplicate keys. These flat envelopes must reject them.
function rejectDuplicateKeys(text: string): void {
  let depth = 0, keyExpected = false;
  const keys = new Set<string>();
  for (const [token] of text.matchAll(/"(?:\\.|[^"\\])*"|[{}\[\]:,]/g)) {
    if (token === '{' || token === '[') { depth += 1; if (depth === 1) keyExpected = true; }
    else if (token === '}' || token === ']') depth -= 1;
    else if (token === ',' && depth === 1) keyExpected = true;
    else if (token.startsWith('"') && depth === 1 && keyExpected) {
      const key: string = JSON.parse(token);
      if (keys.has(key)) throw new Error('Duplicate checkpoint field.');
      keys.add(key);
      keyExpected = false;
    }
  }
}

export class CheckpointSession {
  #module: WebAssembly.Module;
  #convoy: Convoy | ConfiguredConvoy;
  #condition: ConvoyCondition | undefined;
  #digest: string | undefined;
  #actions: Exclude<Action, 0>[] = [];
  #overflow = false;
  #busy = false;
  #freshVisit = true;

  private constructor(module: WebAssembly.Module, convoy: Convoy | ConfiguredConvoy, condition: ConvoyCondition | undefined, digest: string | undefined) {
    this.#module = module;
    this.#convoy = convoy;
    this.#condition = condition;
    this.#digest = digest;
  }

  static async fromBytes(bytes: BufferSource, condition?: ConvoyCondition): Promise<CheckpointSession> {
    return this.#create(bytes, condition, false);
  }

  static async fromConfiguredBytes(bytes: BufferSource): Promise<CheckpointSession> {
    return this.#create(bytes, undefined, true);
  }

  static async #create(bytes: BufferSource, condition: ConvoyCondition | undefined, configured: boolean): Promise<CheckpointSession> {
    // Copy the exact supplied range so compilation and identity use the same bytes.
    const source = ArrayBuffer.isView(bytes)
      ? new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength)
      : new Uint8Array(bytes);
    const retained = new Uint8Array(source);
    const module = await WebAssembly.compile(retained);
    const instance = await WebAssembly.instantiate(module, {});
    const convoy = configured ? new ConfiguredConvoy(instance) : new Convoy(instance, condition);
    let digest: string | undefined;
    if (globalThis.crypto?.subtle) {
      try {
        const hash = await globalThis.crypto.subtle.digest('SHA-256', retained);
        digest = Array.from(new Uint8Array(hash), byte => byte.toString(16).padStart(2, '0')).join('');
      } catch {
        // Ordinary play remains available when the browser cannot provide identity.
      }
    }
    return new CheckpointSession(module, convoy, condition, digest);
  }

  act(action: Action): number {
    if (this.#busy) throw new Error('A checkpoint is loading. Wait before choosing an action.');
    // Configured sessions keep save available by admitting the replay input first.
    if (this.#convoy instanceof ConfiguredConvoy) admitConfiguredInput(this.#convoy, this.#actions.length, action, this.#freshVisit);
    const status = this.#convoy.act(action);
    if (status === 0) {
      this.#freshVisit = visitAfter(action, this.#freshVisit);
      if (action === 0) {
        this.#actions = [];
        this.#overflow = false;
      } else if (this.#actions.length < MAX_ACTIONS) {
        this.#actions.push(action);
      } else {
        this.#overflow = true;
      }
    }
    return status;
  }

  snapshot() {
    return this.#convoy.snapshot();
  }

  developerTrace(): string {
    return this.#convoy.developerTrace();
  }

  get busy(): boolean {
    return this.#busy;
  }

  get canSave(): boolean {
    return !this.#busy && this.#digest !== undefined && !this.#overflow;
  }

  get checkpointMessage(): string {
    if (this.#busy) return 'Loading checkpoint…';
    if (this.#digest === undefined) return 'Checkpoints unavailable: this browser cannot provide Web Crypto SHA-256. Ordinary play is available.';
    if (this.#overflow) return 'Checkpoint saving unavailable after 4096 successful choices. Restart or load an earlier checkpoint to resume saving.';
    return 'Save supports up to 4096 successful choices in this build. Loading requires a compatible file.';
  }

  saveCheckpoint(): string {
    if (!this.canSave) throw new Error(this.checkpointMessage);
    return JSON.stringify({
      magic: MAGIC,
      schema: this.#convoy instanceof ConfiguredConvoy ? 2 : 1,
      exactWasmSHA256: this.#digest,
      ...(this.#convoy instanceof ConfiguredConvoy ? { profileId: this.#convoy.configuration.profileId } : { condition: this.#condition ?? 'current' }),
      actions: this.#actions,
    });
  }

  async loadCheckpoint(file: Blob): Promise<void> {
    if (this.#busy) throw new Error('A checkpoint is already loading.');
    this.#busy = true;
    try {
      if (this.#digest === undefined) throw new Error('Checkpoint loading unavailable: Web Crypto SHA-256 is required.');
      if (!Number.isInteger(file.size) || file.size < 0 || file.size > MAX_BYTES) throw new Error('Checkpoint exceeds the 64 KiB file limit.');
      const bytes = await file.arrayBuffer();
      if (bytes.byteLength > MAX_BYTES) throw new Error('Checkpoint exceeds the 64 KiB file limit.');
      const text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes);
      const checkpoint: unknown = JSON.parse(text);
      rejectDuplicateKeys(text);
      if (checkpoint === null || typeof checkpoint !== 'object' || Array.isArray(checkpoint)) throw new Error('Invalid checkpoint object.');
      const value = checkpoint as Record<string, unknown>;
      const configured = this.#convoy instanceof ConfiguredConvoy;
      const keys = configured ? CONFIGURED_KEYS : KEYS;
      if (Object.keys(value).length !== keys.length || !keys.every(key => Object.hasOwn(value, key))) throw new Error('Invalid checkpoint fields.');
      if (value.magic !== MAGIC || value.schema !== (configured ? 2 : 1)) throw new Error('Unsupported checkpoint format or schema.');
      if (value.exactWasmSHA256 !== this.#digest) throw new Error('Checkpoint belongs to a different Wasm build.');
      if (this.#convoy instanceof ConfiguredConvoy) {
        if (value.profileId !== this.#convoy.configuration.profileId) throw new Error('Checkpoint belongs to a different configured profile.');
      } else if (value.condition !== (this.#condition ?? 'current')) throw new Error('Checkpoint belongs to a different convoy condition.');
      if (!Array.isArray(value.actions) || value.actions.length > MAX_ACTIONS || !value.actions.every(action => Number.isInteger(action) && action >= 1 && action <= 6)) {
        throw new Error('Invalid checkpoint actions; expected at most 4096 commands numbered 1 through 6.');
      }
      const actions = value.actions as Exclude<Action, 0>[];
      const instance = await WebAssembly.instantiate(this.#module, {});
      const candidate = configured ? new ConfiguredConvoy(instance) : new Convoy(instance, this.#condition);
      let freshVisit = true;
      for (const [count, action] of actions.entries()) {
        if (candidate instanceof ConfiguredConvoy) admitConfiguredInput(candidate, count, action, freshVisit);
        if (candidate.act(action) !== 0) throw new Error('Checkpoint contains an unsuccessful command sequence.');
        freshVisit = visitAfter(action, freshVisit);
      }
      candidate.snapshot();
      this.#convoy = candidate;
      this.#actions = actions;
      this.#overflow = false;
      this.#freshVisit = freshVisit;
    } finally {
      this.#busy = false;
    }
  }
}

export async function loadCheckpointConvoy(url: URL | string, condition?: ConvoyCondition, configured = false): Promise<CheckpointSession> {
  if (configured && condition !== undefined) throw new Error('Configured and legacy profiles cannot be combined.');
  let response: Response;
  try {
    response = await fetch(url);
  } catch (error) {
    throw new Error('Could not fetch convoy.wasm. Check the connection and reload.', { cause: error });
  }
  if (!response.ok) throw new Error(`Could not fetch convoy.wasm: HTTP ${response.status}.`);
  try {
    const bytes = await response.arrayBuffer();
    return configured ? await CheckpointSession.fromConfiguredBytes(bytes) : await CheckpointSession.fromBytes(bytes, condition);
  } catch (error) {
    throw new Error(`Could not start convoy.wasm: ${error instanceof Error ? error.message : String(error)}`, { cause: error });
  }
}
