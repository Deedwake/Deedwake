import { Convoy } from './convoy.ts';
import type { Action, ConvoyCondition } from './convoy.ts';

const MAGIC = 'deedwake-convoy-checkpoint';
const MAX_ACTIONS = 4096;
const MAX_BYTES = 64 * 1024;
const KEYS = ['magic', 'schema', 'exactWasmSHA256', 'condition', 'actions'];

export class CheckpointSession {
  #module: WebAssembly.Module;
  #convoy: Convoy;
  #condition: ConvoyCondition | undefined;
  #digest: string | undefined;
  #actions: Exclude<Action, 0>[] = [];
  #overflow = false;
  #busy = false;

  private constructor(module: WebAssembly.Module, convoy: Convoy, condition: ConvoyCondition | undefined, digest: string | undefined) {
    this.#module = module;
    this.#convoy = convoy;
    this.#condition = condition;
    this.#digest = digest;
  }

  static async fromBytes(bytes: BufferSource, condition?: ConvoyCondition): Promise<CheckpointSession> {
    // Copy the exact supplied range so compilation and identity use the same bytes.
    const source = ArrayBuffer.isView(bytes)
      ? new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength)
      : new Uint8Array(bytes);
    const retained = new Uint8Array(source);
    const module = await WebAssembly.compile(retained);
    const convoy = new Convoy(await WebAssembly.instantiate(module, {}), condition);
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
    const status = this.#convoy.act(action);
    if (status === 0) {
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
      schema: 1,
      exactWasmSHA256: this.#digest,
      condition: this.#condition ?? 'current',
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
      if (checkpoint === null || typeof checkpoint !== 'object' || Array.isArray(checkpoint)) throw new Error('Invalid checkpoint object.');
      const value = checkpoint as Record<string, unknown>;
      if (Object.keys(value).length !== KEYS.length || !KEYS.every(key => Object.hasOwn(value, key))) throw new Error('Invalid checkpoint fields.');
      if (value.magic !== MAGIC || value.schema !== 1) throw new Error('Unsupported checkpoint format or schema.');
      if (value.exactWasmSHA256 !== this.#digest) throw new Error('Checkpoint belongs to a different Wasm build.');
      if (value.condition !== (this.#condition ?? 'current')) throw new Error('Checkpoint belongs to a different convoy condition.');
      if (!Array.isArray(value.actions) || value.actions.length > MAX_ACTIONS || !value.actions.every(action => Number.isInteger(action) && action >= 1 && action <= 6)) {
        throw new Error('Invalid checkpoint actions; expected at most 4096 commands numbered 1 through 6.');
      }
      const actions = value.actions as Exclude<Action, 0>[];
      const candidate = new Convoy(await WebAssembly.instantiate(this.#module, {}), this.#condition);
      for (const action of actions) {
        if (candidate.act(action) !== 0) throw new Error('Checkpoint contains an unsuccessful command sequence.');
      }
      candidate.snapshot();
      this.#convoy = candidate;
      this.#actions = actions;
      this.#overflow = false;
    } finally {
      this.#busy = false;
    }
  }
}

export async function loadCheckpointConvoy(url: URL | string, condition?: ConvoyCondition): Promise<CheckpointSession> {
  let response: Response;
  try {
    response = await fetch(url);
  } catch (error) {
    throw new Error('Could not fetch convoy.wasm. Check the connection and reload.', { cause: error });
  }
  if (!response.ok) throw new Error(`Could not fetch convoy.wasm: HTTP ${response.status}.`);
  try {
    return await CheckpointSession.fromBytes(await response.arrayBuffer(), condition);
  } catch (error) {
    throw new Error(`Could not start convoy.wasm: ${error instanceof Error ? error.message : String(error)}`, { cause: error });
  }
}
