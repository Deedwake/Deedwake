import { ACTIONS } from './convoy.ts';
import { Fixture, loadFixture } from './fixture.ts';
import type { Action } from './fixture.ts';

const RESOURCES = ['records', 'text_bytes', 'journal', 'actions', 'pending', 'references', 'delivery_sequence', 'host_text_bytes'] as const;
export class CapacityError extends Error {
  readonly resource: typeof RESOURCES[number] | 'checkpoint_commands';
  constructor(code: number | 'checkpoint_commands') {
    super('This session has reached its supported capacity. Your progress is unchanged; you can save or restart.');
    this.name = 'CapacityError';
    this.resource = code === 'checkpoint_commands' ? code : RESOURCES[code - 17]!;
  }
}

export type ConvoyConfiguration = {
  profileId: string; observer: 'Mara' | 'Sela'; retention: 'precise' | 'incomplete';
  reaction: 'restrict_until_correction' | 'warn_and_continue';
  selaConfidence: 'trusted' | 'unverified';
  initial: number; routine: number; intermediary: number; traceCapacity: number;
};

export class ConfiguredConvoy extends Fixture {
  readonly configuration: Readonly<ConvoyConfiguration>;
  #command?: (action: number) => number;

  constructor(instance: WebAssembly.Instance) {
    super(instance, ACTIONS);
    const exports = instance.exports;
    for (const [name, arity] of Object.entries({ dw_convoy_configured_init: 0, dw_convoy_config: 1, dw_convoy_profile_len: 0, dw_convoy_profile_byte: 1 })) {
      const fn = exports[name];
      if (typeof fn !== 'function' || fn.length !== arity) throw new Error(`Missing or incompatible Wasm export: ${name}.`);
    }
    if ((exports.dw_convoy_configured_init as () => number)() !== 0) throw new Error('Configured convoy initialization failed.');
    const command = exports.dw_convoy_command;
    const limits = exports.dw_convoy_limits;
    if (command !== undefined || limits !== undefined) {
      if (typeof command !== 'function' || command.length !== 1 || typeof limits !== 'function' || limits.length !== 1) throw new Error('Incompatible convoy limits capability.');
      const expected = [1, 4096, 1048576, 128, 4, 3, 4, 32768, 4096, 131072];
      for (const [field, value] of expected.entries()) {
        if ((limits as (field: number) => number)(field) !== value) throw new Error('Unsupported convoy limits.');
      }
      for (const field of [10, 0xffff_ffff]) {
        if (![0xffff_ffff, -1].includes((limits as (field: number) => number)(field))) throw new Error('Invalid convoy limits bounds.');
      }
      this.#command = command as (action: number) => number;
    }
    const read = exports.dw_convoy_config as (field: number) => number;
    if (read(0) !== 1) throw new Error('Unsupported convoy configuration version.');
    const observer = read(1), retention = read(2), reaction = read(3);
    if (![1, 2].includes(observer) || ![1, 2].includes(retention) || ![1, 2].includes(reaction)) throw new Error('Invalid configured convoy selection.');
    // Earlier configured artifacts only supported trusted verification.
    const confidenceExport = exports.dw_convoy_confidence;
    let confidence = 1;
    if (confidenceExport !== undefined) {
      if (typeof confidenceExport !== 'function' || confidenceExport.length !== 0) throw new Error('Incompatible convoy confidence export.');
      confidence = (confidenceExport as () => number)();
      if (![1, 2].includes(confidence)) throw new Error('Invalid convoy confidence.');
    }
    const initial = read(4), routine = read(5), intermediary = read(6), traceCapacity = read(7);
    for (const delay of [initial, routine, intermediary]) {
      if (!Number.isInteger(delay) || delay < 0 || delay > 1000) throw new Error('Invalid configured delivery delay.');
    }
    if (!Number.isInteger(traceCapacity) || traceCapacity < 0 || traceCapacity > 1024) throw new Error('Invalid configured trace capacity.');
    for (const field of [8, 0xffff_ffff]) {
      if (![0xffff_ffff, -1].includes(read(field))) throw new Error('Invalid configured metadata bounds.');
    }
    const length = (exports.dw_convoy_profile_len as () => number)();
    if (!Number.isInteger(length) || length < 1 || length > 64) throw new Error('Invalid convoy profile length.');
    const byte = exports.dw_convoy_profile_byte as (index: number) => number;
    const bytes = new Uint8Array(length);
    for (let index = 0; index < length; index += 1) {
      const value = byte(index);
      if (!Number.isInteger(value) || value < 0 || value > 255) throw new Error('Invalid convoy profile byte.');
      bytes[index] = value;
    }
    if (byte(length) !== 256 || byte(0xffff_ffff) !== 256) throw new Error('Invalid convoy profile bounds.');
    const profileId = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    if (!/^[a-z0-9_-]{1,64}$/.test(profileId)) throw new Error('Invalid convoy profile identity.');
    this.configuration = Object.freeze({ profileId, observer: observer === 1 ? 'Mara' : 'Sela', retention: retention === 1 ? 'precise' : 'incomplete', reaction: reaction === 1 ? 'restrict_until_correction' : 'warn_and_continue', selaConfidence: confidence === 1 ? 'trusted' : 'unverified', initial, routine, intermediary, traceCapacity });
  }

  override act(action: Action): number {
    if (!Number.isInteger(action) || action < 0 || action > 6) throw new Error(`Invalid Wasm action: ${action}.`);
    if (!this.#command) return super.act(action);
    const result = this.#command(action);
    if (!Number.isInteger(result)) throw new Error('Invalid configured command status.');
    if (result >= 17 && result <= 24) throw new CapacityError(result);
    if (result < 0 || result > 2) throw new Error('Invalid configured command status.');
    return result;
  }

  override snapshot() {
    const view = super.snapshot();
    const { observer, routine, intermediary } = this.configuration;
    const timing = (ticks: number) => ticks === 0 ? 'immediate' : `${ticks} fictional ${ticks === 1 ? 'hour' : 'hours'}`;
    view.choices[1]!.label = `Ask ${observer} what she saw`;
    view.choices[2]!.label = `Release convoy · take receipt · ${timing(routine)}`;
    view.choices[3]!.label = `Give receipt to Sela · intermediary pouch · ${timing(intermediary)}`;
    return view;
  }
}

export function loadConfiguredConvoy(url: URL | string): Promise<ConfiguredConvoy> {
  return loadFixture(url, 'convoy.wasm', instance => new ConfiguredConvoy(instance));
}
