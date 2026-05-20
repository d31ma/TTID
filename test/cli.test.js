import { describe, expect, test } from 'bun:test';
import {
    executeMachineOperation,
    machineErrorResponse,
    machineSuccessResponse
} from '../src/cli/machine.js';

describe('machine interface', () => {
    test('generates a TTID through the machine interface', () => {
        const request = {
            requestId: 'generate-1',
            op: 'generate'
        };

        const result = executeMachineOperation(request);
        const payload = machineSuccessResponse(request, Date.now(), result);

        expect(payload.ok).toBe(true);
        expect(payload.protocolVersion).toBe(1);
        expect(payload.op).toBe('generate');
        expect(payload.requestId).toBe('generate-1');
        expect(typeof payload.result).toBe('string');
        expect(String(payload.result)).toHaveLength(11);
    });

    test('updates and deletes an existing TTID through arguments', () => {
        const created = executeMachineOperation({ op: 'generate' });
        const updated = executeMachineOperation({ op: 'generate', id: created });
        const deleted = executeMachineOperation({ op: 'generate', id: updated, delete: true });

        expect(String(updated).split('-')).toHaveLength(2);
        expect(String(deleted).split('-')).toHaveLength(3);
    });

    test('decodes timestamps through the machine interface', () => {
        const id = String(executeMachineOperation({ op: 'generate' }));
        const result = executeMachineOperation({
            requestId: 'decode-1',
            op: 'decodeTime',
            id
        });

        expect(result).toEqual({
            createdAt: expect.any(Number)
        });
    });

    test('validates TTIDs through the machine interface', () => {
        const id = String(executeMachineOperation({ op: 'generate' }));
        const result = executeMachineOperation({
            op: 'isTTID',
            id
        });

        expect(result).toEqual({
            valid: true,
            createdAt: expect.any(Number)
        });
    });

    test('returns structured errors', () => {
        const request = {
            requestId: 'bad-generate',
            op: 'generate',
            id: 'not-a-valid-ttid'
        };

        let error;
        try {
            executeMachineOperation(request);
        } catch (caught) {
            error = caught;
        }

        const payload = machineErrorResponse(request, Date.now(), error);
        expect(payload.ok).toBe(false);
        expect(payload.requestId).toBe('bad-generate');
        expect(payload.error.message).toBe('Invalid TTID!');
    });

    test('rejects unsupported operations through the machine interface', () => {
        expect(() =>
            executeMachineOperation({
                op: 'unknownOperation'
            })
        ).toThrow('Unsupported machine operation "unknownOperation"');
    });
});
