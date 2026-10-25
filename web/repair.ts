import { Fixture, loadFixture } from './fixture.ts';
import { readDeveloperCheck, runDeveloperCheck } from './checks.ts';

export const ACTIONS = [
  { id: 1, label: 'Submit donated appliance report' },
  { id: 2, label: 'Inspect the donated appliance' },
  { id: 3, label: 'Request cable repair' },
  { id: 4, label: 'Attempt collection' },
  { id: 5, label: 'Ask at intake' },
  { id: 6, label: 'Advance fictional time · 1 hour' },
] as const;

const ACTION_KINDS = ['inspect', 'repair', 'collect'] as const;
const DISPOSITIONS = ['no_proposal', 'would_reject', 'ready_to_execute'] as const;
const REASONS = ['report_not_arrived', 'already_diagnosed', 'diagnosis_not_arrived', 'already_repaired', 'ready_not_arrived', 'already_collected', 'cable_receipt_not_arrived', 'prior_rejection_unchanged', 'repair_not_executed', 'prerequisites_met'] as const;
const FACTS = ['donor_report', 'diagnosis', 'cable_receipt', 'ready', 'collected'] as const;
const SOURCES = ['player', 'donor', 'intake', 'technician', 'stock', 'collection'] as const;
const STATUSES = ['proposed', 'accepted_unconfirmed', 'host_rejected', 'confirmed'] as const;

function bounded(value: number, maximum: number, name: string): number {
  if (!Number.isInteger(value) || value < 0 || value > maximum) {
    throw new Error(`Invalid Wasm repair inspection ${name}: ${value}.`);
  }
  return value;
}

function code<T extends string>(value: number, values: readonly T[], name: string): T {
  bounded(value, values.length, name);
  if (value === 0) throw new Error(`Invalid Wasm repair inspection ${name}: 0.`);
  return values[value - 1]!;
}

export class Repair extends Fixture {
  #instance: WebAssembly.Instance;

  constructor(instance: WebAssembly.Instance) {
    super(instance, ACTIONS);
    this.#instance = instance;
  }

  runDeveloperCheck(caseId: number) {
    return runDeveloperCheck(this.#instance.exports, caseId);
  }

  developerCheckReport() {
    return readDeveloperCheck(this.#instance.exports);
  }

  developerInspection() {
    const inspect = this.#instance.exports.dw_repair_inspect;
    if (typeof inspect !== 'function' || inspect.length !== 3) {
      throw new Error('Missing or incompatible Wasm export: dw_repair_inspect.');
    }
    const read = (section: number, row: number, column = 0): number => {
      const value = inspect(section, row, column) as number;
      return Number.isInteger(value) && value >= -0x8000_0000 && value < 0 ? value + 0x1_0000_0000 : value;
    };
    const absent = (section: number, row: number, column = 0): void => {
      const value = read(section, row, column);
      if (value !== -1 && value !== 0xffff_ffff) {
        throw new Error(`Invalid Wasm repair inspection bounds: ${section}/${row}/${column}.`);
      }
    };
    if (read(0, 0) !== 1) throw new Error('Unsupported Wasm repair inspection version; expected 1.');
    const time = bounded(read(0, 1), 1000, 'time');
    const droppedTrace = bounded(read(0, 2), 0xffff_fffe, 'dropped trace count');
    const actionCount = bounded(read(0, 3), 4, 'action count');
    if (read(1, 0) !== 1) throw new Error('Unsupported Wasm repair inspection schema version; expected 1.');
    const configuration = {
      schema_version: 1 as const,
      policy_set: code(read(1, 1), ['repair_cafe_v1'] as const, 'policy set'),
      trace_capacity: bounded(read(1, 2), 1024, 'trace capacity'),
      report_to_intake_ticks: bounded(read(1, 3), 1000, 'report_to_intake_ticks'),
      report_to_technician_ticks: bounded(read(1, 4), 1000, 'report_to_technician_ticks'),
      technician_to_intake_ticks: bounded(read(1, 5), 1000, 'technician_to_intake_ticks'),
      technician_to_collection_ticks: bounded(read(1, 6), 1000, 'technician_to_collection_ticks'),
      cable_receipt_at_tick: bounded(read(1, 7), 1000, 'cable_receipt_at_tick'),
    };
    const decisions = ACTION_KINDS.map((action, row) => {
      const disposition = code(read(2, row, 0), DISPOSITIONS, 'disposition');
      const reason = code(read(2, row, 1), REASONS, 'reason');
      const causeCount = bounded(read(2, row, 2), 2, 'cause count');
      const causes = Array.from({ length: causeCount }, (_, index) => ({
        fact: code(read(3, row * 2 + index, 0), FACTS, 'cause fact'),
        source: code(read(3, row * 2 + index, 1), SOURCES, 'cause source'),
        time: bounded(read(3, row * 2 + index, 2), time, 'cause creation time'),
      }));
      for (let index = causeCount; index < 2; index += 1) {
        for (let column = 0; column < 3; column += 1) absent(3, row * 2 + index, column);
      }
      return { action, disposition, reason, causes };
    });
    const actions = Array.from({ length: actionCount }, (_, row) => {
      const action = code(read(4, row, 0), ACTION_KINDS, 'action kind');
      const status = code(read(4, row, 1), STATUSES, 'action status');
      const rejection = bounded(read(4, row, 2), REASONS.length, 'rejection reason');
      const confirmed = bounded(read(4, row, 3), FACTS.length, 'confirmed fact');
      if ((status === 'confirmed') !== (confirmed !== 0) || (status === 'host_rejected') !== (rejection !== 0)) {
        throw new Error('Invalid Wasm repair inspection action status fields.');
      }
      return { action, status, rejection: rejection === 0 ? null : REASONS[rejection - 1]!, confirmed: confirmed === 0 ? null : FACTS[confirmed - 1]! };
    });
    for (let row = actionCount; row < 4; row += 1) {
      for (let column = 0; column < 4; column += 1) absent(4, row, column);
    }
    for (const [section, row, column] of [[0, 4, 0], [0, 0, 1], [1, 8, 0], [1, 0, 1], [2, 3, 0], [2, 0, 3], [3, 6, 0], [3, 0, 3], [4, 4, 0], [4, 0, 4], [5, 0, 0]]) {
      absent(section!, row!, column!);
    }
    return { version: 1 as const, time, droppedTrace, configuration, decisions, actions };
  }
}

export type RepairInspection = ReturnType<Repair['developerInspection']>;

export function loadRepair(url: URL | string): Promise<Repair> {
  return loadFixture(url, 'repair.wasm', instance => new Repair(instance));
}
