import { ACTIONS } from './convoy.ts';
import { Fixture, loadFixture } from './fixture.ts';

export type ConvoyConfiguration = {
  profileId: string; observer: 'Mara' | 'Sela'; retention: 'precise' | 'incomplete';
  reaction: 'restrict_until_correction' | 'warn_and_continue';
  initial: number; routine: number; intermediary: number; traceCapacity: number;
};

export class ConfiguredConvoy extends Fixture {
  readonly configuration: Readonly<ConvoyConfiguration>;

  constructor(instance: WebAssembly.Instance) {
    super(instance, ACTIONS);
    const exports = instance.exports;
    for (const [name, arity] of Object.entries({ dw_convoy_configured_init: 0, dw_convoy_config: 1, dw_convoy_profile_len: 0, dw_convoy_profile_byte: 1 })) {
      const fn = exports[name];
      if (typeof fn !== 'function' || fn.length !== arity) throw new Error(`Missing or incompatible Wasm export: ${name}.`);
    }
    if ((exports.dw_convoy_configured_init as () => number)() !== 0) throw new Error('Configured convoy initialization failed.');
    const read = exports.dw_convoy_config as (field: number) => number;
    if (read(0) !== 1) throw new Error('Unsupported convoy configuration version.');
    const observer = read(1), retention = read(2), reaction = read(3);
    if (![1, 2].includes(observer) || ![1, 2].includes(retention) || ![1, 2].includes(reaction)) throw new Error('Invalid configured convoy selection.');
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
    this.configuration = Object.freeze({ profileId, observer: observer === 1 ? 'Mara' : 'Sela', retention: retention === 1 ? 'precise' : 'incomplete', reaction: reaction === 1 ? 'restrict_until_correction' : 'warn_and_continue', initial, routine, intermediary, traceCapacity });
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
